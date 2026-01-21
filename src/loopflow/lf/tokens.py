"""Token counting and analysis for prompts."""

from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional

import tiktoken

_encoder = tiktoken.get_encoding("cl100k_base")

# Context limit: 120k leaves room for model response.
MAX_SAFE_TOKENS = 120_000


def count_tokens(text: str) -> int:
    """Count tokens in text using Claude's tokenizer."""
    return len(_encoder.encode(text))


@dataclass
class TokenNode:
    """A node in the token tree."""

    name: str
    tokens: int = 0
    children: dict[str, "TokenNode"] = field(default_factory=dict)

    def add_child(self, name: str, tokens: int) -> "TokenNode":
        """Add or update a child node."""
        if name not in self.children:
            self.children[name] = TokenNode(name=name)
        self.children[name].tokens += tokens
        return self.children[name]

    def total_tokens(self) -> int:
        """Total tokens including all descendants."""
        return self.tokens + sum(c.total_tokens() for c in self.children.values())

    def add_path(self, path: list[str], tokens: int) -> None:
        """Add tokens at a nested path (e.g., ["context", "src", "cli", "run.py"])."""
        if not path:
            self.tokens += tokens
            return
        child = self.add_child(path[0], 0)
        child.add_path(path[1:], tokens)


@dataclass
class TokenTree:
    """Hierarchical token breakdown of a prompt."""

    root: TokenNode = field(default_factory=lambda: TokenNode(name="root"))

    def add(self, category: str, name: str, tokens: int, path: Optional[list[str]] = None) -> None:
        """Add tokens under category, optionally nested by path."""
        cat_node = self.root.add_child(category, 0)
        if path:
            cat_node.add_path(path + [name], tokens)
        else:
            cat_node.add_child(name, tokens)

    def total(self) -> int:
        """Total tokens in the tree."""
        return self.root.total_tokens()

    def format(self, threshold_pct: float = 0.05) -> str:
        """Format tree as text with adaptive detail.

        Larger nodes get more breakdown. Nodes under threshold_pct of total
        are collapsed.
        """
        total = self.total()
        if total == 0:
            return "Tokens: 0"

        lines = [f"Tokens: {total:,}", ""]
        max_bar = 20

        # Sort categories by size
        categories = sorted(
            self.root.children.items(),
            key=lambda x: x[1].total_tokens(),
            reverse=True,
        )

        for cat_name, cat_node in categories:
            cat_total = cat_node.total_tokens()
            if cat_total == 0:
                continue

            bar = self._bar_for_tokens(cat_total, total, max_bar)

            lines.append(f"{cat_name:<14} {cat_total:>6,} {bar}")

            # Break down if significant
            if cat_total / total >= threshold_pct:
                self._format_children(
                    cat_node, lines, total, threshold_pct, indent=2, max_bar=max_bar
                )

        return "\n".join(lines)

    def _format_children(
        self,
        node: TokenNode,
        lines: list[str],
        total: int,
        threshold_pct: float,
        indent: int,
        max_bar: int,
    ) -> None:
        """Recursively format children with adaptive detail."""
        children = sorted(
            node.children.items(),
            key=lambda x: x[1].total_tokens(),
            reverse=True,
        )

        # Show top children, roll up small ones
        shown = 0
        rolled_up = 0
        rolled_up_tokens = 0

        for name, child in children:
            child_total = child.total_tokens()
            child_pct = child_total / total

            if shown < 4 or child_pct >= threshold_pct:
                prefix = " " * indent
                bar = self._bar_for_tokens(child_total, total, max_bar)
                name_width = max(1, 14 - indent)
                lines.append(f"{prefix}{name:<{name_width}} {child_total:>6,} {bar}")

                # Recurse if this child is also significant
                if child_pct >= threshold_pct and child.children:
                    self._format_children(
                        child,
                        lines,
                        total,
                        threshold_pct,
                        indent + 2,
                        max_bar,
                    )

                shown += 1
            else:
                rolled_up += 1
                rolled_up_tokens += child_total

        if rolled_up > 0:
            prefix = " " * indent
            bar = self._bar_for_tokens(rolled_up_tokens, total, max_bar)
            pad = max(0, 8 - indent)
            lines.append(f"{prefix}({rolled_up} more){'':<{pad}} {rolled_up_tokens:>6,} {bar}")

    @staticmethod
    def _bar_for_tokens(tokens: int, total: int, max_bar: int) -> str:
        """Render an absolute bar scaled to total tokens."""
        if total <= 0 or tokens <= 0:
            return "▏"
        bar_len = int((tokens / total) * max_bar)
        return "█" * bar_len if bar_len > 0 else "▏"


def analyze_prompt_tokens(
    docs: Optional[list[tuple[Path, str]]] = None,
    diff: Optional[str] = None,
    diff_files: Optional[list[tuple[Path, str]]] = None,
    task: Optional[tuple[str, str]] = None,
    repo_root: Optional[Path] = None,
    clipboard: Optional[str] = None,
    loopflow_doc: Optional[str] = None,
    summaries: Optional[list[tuple[Path, str]]] = None,
) -> TokenTree:
    """Analyze token distribution in prompt components."""
    tree = TokenTree()

    if loopflow_doc:
        tokens = count_tokens(loopflow_doc)
        tree.add("loopflow", "LOOPFLOW.md", tokens)

    if docs:
        for doc_path, content in docs:
            tokens = count_tokens(content)
            tree.add("docs", doc_path.name, tokens)

    if diff:
        tokens = count_tokens(diff)
        tree.add("diff", "branch diff", tokens)

    if diff_files and repo_root:
        for file_path, content in diff_files:
            tokens = count_tokens(content)
            try:
                rel = file_path.relative_to(repo_root)
                parts = list(rel.parts[:-1])  # directory parts
                tree.add("files", rel.name, tokens, path=parts)
            except ValueError:
                tree.add("files", file_path.name, tokens)

    if summaries:
        for summary_path, content in summaries:
            tokens = count_tokens(content)
            tree.add("summaries", str(summary_path), tokens)

    if task:
        name, content = task
        tokens = count_tokens(content)
        tree.add("task", name or "inline", tokens)

    if clipboard:
        tokens = count_tokens(clipboard)
        tree.add("clipboard", "pasted text", tokens)

    return tree


def analyze_components(components) -> TokenTree:
    """Analyze token distribution from PromptComponents."""
    # Extract text from ClipboardContent if present
    clipboard_text = components.clipboard.text if components.clipboard else None
    return analyze_prompt_tokens(
        docs=components.docs,
        diff=components.diff,
        diff_files=components.diff_files,
        task=components.step,
        repo_root=components.repo_root,
        clipboard=clipboard_text,
        loopflow_doc=components.loopflow_doc,
        summaries=components.summaries,
    )
