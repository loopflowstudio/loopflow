"""Tests for loopflow.tokens module."""

from pathlib import Path

from loopflow.tokens import count_tokens, TokenNode, TokenTree, analyze_prompt_tokens


def test_count_tokens_returns_int():
    """count_tokens returns a positive integer for text."""
    result = count_tokens("hello world")
    assert isinstance(result, int)
    assert result > 0


def test_count_tokens_empty_string():
    """count_tokens returns 0 for empty string."""
    result = count_tokens("")
    assert result == 0


def test_count_tokens_scales_with_length():
    """Longer text produces more tokens."""
    short = count_tokens("hello")
    long = count_tokens("hello " * 100)
    assert long > short


def test_token_node_add_child():
    """TokenNode tracks children with token counts."""
    node = TokenNode(name="root")
    child = node.add_child("docs", 100)

    assert "docs" in node.children
    assert child.tokens == 100


def test_token_node_add_child_accumulates():
    """Adding to same child accumulates tokens."""
    node = TokenNode(name="root")
    node.add_child("docs", 100)
    node.add_child("docs", 50)

    assert node.children["docs"].tokens == 150


def test_token_node_total_tokens():
    """total_tokens sums node and all descendants."""
    node = TokenNode(name="root", tokens=10)
    node.add_child("a", 100)
    node.add_child("b", 50)

    assert node.total_tokens() == 160


def test_token_node_add_path():
    """add_path creates nested structure."""
    node = TokenNode(name="root")
    node.add_path(["src", "cli", "run.py"], 100)

    assert "src" in node.children
    assert "cli" in node.children["src"].children
    assert "run.py" in node.children["src"].children["cli"].children
    assert node.children["src"].children["cli"].children["run.py"].tokens == 100


def test_token_tree_add():
    """TokenTree organizes items by category."""
    tree = TokenTree()
    tree.add("docs", "README.md", 100)
    tree.add("docs", "STYLE.md", 50)
    tree.add("task", "implement", 25)

    assert tree.total() == 175
    assert tree.root.children["docs"].total_tokens() == 150
    assert tree.root.children["task"].total_tokens() == 25


def test_token_tree_add_with_path():
    """TokenTree supports nested paths for files."""
    tree = TokenTree()
    tree.add("context", "run.py", 100, path=["src", "cli"])

    context = tree.root.children["context"]
    assert context.children["src"].children["cli"].children["run.py"].tokens == 100


def test_token_tree_format_shows_totals():
    """format() produces readable output with totals."""
    tree = TokenTree()
    tree.add("docs", "README.md", 1000)
    tree.add("context", "main.py", 500)

    output = tree.format()

    assert "1,500" in output  # Total
    assert "docs" in output
    assert "context" in output


def test_token_tree_format_empty():
    """format() handles empty tree."""
    tree = TokenTree()
    output = tree.format()

    assert "0" in output


def test_analyze_prompt_tokens_docs():
    """analyze_prompt_tokens handles docs."""
    docs = [(Path("README.md"), "# Project\n\nDescription.")]
    tree = analyze_prompt_tokens(docs=docs)

    assert tree.total() > 0
    assert "docs" in tree.root.children


def test_analyze_prompt_tokens_diff():
    """analyze_prompt_tokens handles diff."""
    tree = analyze_prompt_tokens(diff="+ added line\n- removed line")

    assert tree.total() > 0
    assert "diff" in tree.root.children


def test_analyze_prompt_tokens_task():
    """analyze_prompt_tokens handles task."""
    tree = analyze_prompt_tokens(task=("implement", "Build the feature."))

    assert tree.total() > 0
    assert "task" in tree.root.children


def test_analyze_prompt_tokens_context_files():
    """analyze_prompt_tokens handles context files with repo_root."""
    repo_root = Path("/project")
    context_files = [
        (Path("/project/src/main.py"), "print('hello')"),
        (Path("/project/tests/test_main.py"), "def test_main(): pass"),
    ]

    tree = analyze_prompt_tokens(context_files=context_files, repo_root=repo_root)

    assert tree.total() > 0
    assert "context" in tree.root.children


def test_analyze_prompt_tokens_combined():
    """analyze_prompt_tokens handles all components together."""
    tree = analyze_prompt_tokens(
        docs=[(Path("README.md"), "# Readme")],
        diff="+ line",
        task=("implement", "task content"),
        context_files=[(Path("/p/main.py"), "code")],
        repo_root=Path("/p"),
    )

    # All categories present
    assert "docs" in tree.root.children
    assert "diff" in tree.root.children
    assert "task" in tree.root.children
    assert "context" in tree.root.children


def test_analyze_prompt_tokens_clipboard():
    """analyze_prompt_tokens handles clipboard content."""
    tree = analyze_prompt_tokens(clipboard="error traceback from terminal")

    assert tree.total() > 0
    assert "clipboard" in tree.root.children
