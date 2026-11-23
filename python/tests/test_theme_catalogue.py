from scripts import theme_catalogue


def test_committed_themes_equal_a_fresh_run() -> None:
    assert theme_catalogue.check() == []

