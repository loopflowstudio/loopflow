"""Token counting and analysis for prompts."""

from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional

import tiktoken


_encoder = tiktoken.get_encoding("cl100k_base")


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
        """Add tokens to the tree.

        Args:
            category: Top-level category (docs, diff, task, context, arg)
            name: Display name for this item
            tokens: Token count
            path: Optional path within category (for nested file structure)
        """
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

            pct = cat_total / total
            bar_len = int(pct * max_bar)
            bar = "█" * bar_len if bar_len > 0 else "▏"

            lines.append(f"{cat_name:<14} {cat_total:>6,} {bar}")

            # Break down if significant
            if pct >= threshold_pct:
                self._format_children(cat_node, lines, total, threshold_pct, indent=2)

        return "\n".join(lines)

    def _format_children(
        self,
        node: TokenNode,
        lines: list[str],
        total: int,
        threshold_pct: float,
        indent: int,
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
                lines.append(f"{prefix}{name:<{14-indent}} {child_total:>6,}")

                # Recurse if this child is also significant
                if child_pct >= threshold_pct and child.children:
                    self._format_children(child, lines, total, threshold_pct, indent + 2)

                shown += 1
            else:
                rolled_up += 1
                rolled_up_tokens += child_total

        if rolled_up > 0:
            prefix = " " * indent
            lines.append(f"{prefix}({rolled_up} more){'':<{8-indent}} {rolled_up_tokens:>6,}")


def analyze_prompt_tokens(
    docs: Optional[list[tuple[Path, str]]] = None,
    diff: Optional[str] = None,
    task: Optional[tuple[str, str]] = None,
    context_files: Optional[list[tuple[Path, str]]] = None,
    repo_root: Optional[Path] = None,
) -> TokenTree:
    """Analyze token distribution in prompt components."""
    tree = TokenTree()

    if docs:
        for doc_path, content in docs:
            tokens = count_tokens(content)
            tree.add("docs", doc_path.name, tokens)

    if diff:
        tokens = count_tokens(diff)
        tree.add("diff", "branch diff", tokens)

    if task:
        name, content = task
        tokens = count_tokens(content)
        tree.add("task", name or "inline", tokens)

    if context_files and repo_root:
        for file_path, content in context_files:
            tokens = count_tokens(content)
            try:
                rel = file_path.relative_to(repo_root)
                parts = list(rel.parts[:-1])  # directory parts
                tree.add("context", rel.name, tokens, path=parts)
            except ValueError:
                tree.add("context", file_path.name, tokens)

    return tree


def analyze_components(components) -> TokenTree:
    """Analyze token distribution from PromptComponents."""
    return analyze_prompt_tokens(
        docs=components.docs,
        diff=components.diff,
        task=components.task,
        context_files=components.context_files,
        repo_root=components.repo_root,
    )
