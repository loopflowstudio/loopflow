# What people say they like in terminal UX (LOO-381 background)

Jack Heart requested this on 2026-10-05 during the LOO-381 demo. A research
agent read about 25 sources: Hacker News threads and comment searches, three
GitHub discussions, five blog posts and two Mitchell Hashimoto posts.

Limits: Reddit and lobste.rs blocked fetching, so this is HN-heavy. Pages were
read through a summarizer; six quotes were re-checked against full text
(softirq, lexicality, hbn, commandersaki, jitl, blondin). Treat the rest as
probably exact, unverified. Nothing here is from Loopflow users.

## Warp

Liked:

- **Separation of prompt, input and output.** lexicality: "Your prompt, input
  and command output are entirely separate entities rather than one single
  stream of text. It's like using a Jupyter notebook"
  ([HN](https://news.ycombinator.com/item?id=49173766)).
- **Readability and copy.** fnordlord: blocks are "cool for readability and
  copy/pasting" ([HN](https://news.ycombinator.com/item?id=42248914)). Louie
  Berwanger: "pairing inputs with their outputs and highlighting them as you go
  back" ([blog](https://spin.atomicobject.com/warp-terminal/)).
- **Prompt stays visible while scrolling**, and pinned output (blopker,
  [HN](https://news.ycombinator.com/item?id=33912626)).
- **Input behaves like a Mac text field**: Cmd/Option arrows, shift-select,
  click to place the cursor (hbn,
  [HN](https://news.ycombinator.com/item?id=49174665)).

Disliked:

- **Block chrome getting in the way of selection and focus.** BoorishBears:
  selecting text then right-clicking highlights the whole block
  ([HN](https://news.ycombinator.com/item?id=33912103)). ishaanbahal: "click to
  focus on blocks becomes a hindrance… I hardly care for the previous blocks"
  ([HN](https://news.ycombinator.com/item?id=41224492)).
- **Magic that breaks.** softirq: "commands in Warp going haywire because of how
  much magic there is in creating visual blocks"
  ([HN](https://news.ycombinator.com/item?id=39466502)). Blocks and input do not
  work inside tmux ([Warp discussion](https://github.com/warpdotdev/Warp/discussions/501)).
- **Replacing the shell's line editor**: missing `ctrl+x ctrl+e`, no shell
  completion scripts ([HN](https://news.ycombinator.com/item?id=37571713),
  [HN](https://news.ycombinator.com/item?id=30921231)).
- **Login and AI push** ([HN](https://news.ycombinator.com/item?id=42247583),
  [HN](https://news.ycombinator.com/item?id=49172869)).

Used versus ignored: fnordlord, "I honestly don't use many of their features".
No first-hand praise was found for sticky headers, failure coloring or per-block
filtering; those appeared only in feature lists.

## Ghostty

- **Native and fast together.** jitl: "Ghostty feels like a Mac app like iTerm2
  while being fast… Wezterm feels more like an app ported from Linux"
  ([HN](https://news.ycombinator.com/item?id=42518710)). Mitchell's stated goal:
  "fast, feature-rich, and have a platform-native GUI"
  ([post](https://mitchellh.com/writing/ghostty-is-coming)).
- **Text rendering** (Jarred, [HN](https://news.ycombinator.com/item?id=42517447)).
- **Defaults.** neobrain: "defaults are good enough that you barely need it"
  ([HN](https://news.ycombinator.com/item?id=46575435)).
- **Theming and `minimum-contrast`**
  ([blog](https://www.jonashietala.se/blog/2025/01/06/first_impressions_of_ghostty/)).
- **Shell integration**: "triple click to select all output of the last command"
  (commandersaki, [HN](https://news.ycombinator.com/item?id=47211303)).
- Missed: scrollback search at 1.0, a settings UI, iTerm's tmux integration,
  keyboard pane moves. Almost nobody cites jump-to-prompt as a reason to like it.

## Others (one or two quotes each)

iTerm2: feature depth and Mac-ness. WezTerm: built-in multiplexer, Lua config
both powerful and annoying. kitty: speed plus features, but "felt like a linux
app that was on macos". Alacritty: minimal and fast, left over fonts. Wave: one
user has long wanted "each command and its output is a separate box"; others
object to Electron. No usable evidence for Zed, Tabby or Rio.

## Blocks on OSC 133

Two Ghostty discussions ask for Warp-style blocks on existing OSC 133 marks:
separators, an exit-status stripe, one-click copy, navigation, collapse, filter
([7607](https://github.com/ghostty-org/ghostty/discussions/7607),
[11786](https://github.com/ghostty-org/ghostty/discussions/11786)). Collaborator
pluiedev: "We already support semantic prompt regions… the question is what is
the best approach to visually present them", preferring minimal status lines.
Mitchell is interested but wants a core block iterator first.

## Embedded terminals

Mitchell: "Most of these implementations are incomplete, buggy, and slow"
([post](https://mitchellh.com/writing/libghostty-is-coming)). The value is not
switching windows. The complaints: keybindings that differ from a real terminal
(`alt+backspace` in VS Code), "overly clever and brittle" input interception,
and an environment that differs from the host's. Latency is contested. Nothing
was found on agent-tool terminals specifically.

## Gaps

No user quotes on padding or line height; typography talk was about font
rendering and ligatures. Jack's spacing and header-size preferences rest on his
own eye, not on this.

## Reading for LOO-381 (interpretation, not Jack's)

- What people keep citing is separation and whole-output copy. That is the part
  LOO-381 already builds.
- The recurring irritation is block chrome that takes over selection or focus.
  The branch's rule (pointer events always reach the terminal; a block is
  selected only on a click with no drag and no text selection; one selection at
  a time) is aimed at exactly this, and the BoorishBears right-click case is
  worth adding to the display test.
- Warp's losses came from replacing the shell line and from AI crowding the
  terminal, not from blocks. Loopflow keeps the shell's own line editor and
  builds on OSC 133, which also keeps it close to where Ghostty upstream may go.
- The embedded-terminal complaint about environment parity is the color bug
  this Task started from.
- Sharing, filtering, bookmarks and collapse are marketed more than cited; the
  design's deferral of them has support here.
