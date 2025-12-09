# Loopflow

Arrange LLMs to code in harmony.

## Installation

`pip install loopflow`

## Basic Usage

`lf claude review`: Launches a claude code session with a default initial prompt. Should Include:
* README and STYLE info from the entire codebase
* A default "perspective" file, i.e. something like `.lf/perspectives/default.lf`
* The contents of the task, e.g. something like `.lf/tasks/review.lf`

`lf claude review -p`: Uses claude's print mode, which is to say:
* Also wraps the claude code in our commit wrapper to make it easier to make atomic in git

`lf claude review -p artist`: Uses `.lf/perspectives/artist.lf` or something
