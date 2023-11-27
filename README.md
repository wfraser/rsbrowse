rsbrowse
========

Browse Rust code from the compiler's perspective.

[![demo!](https://asciinema.org/a/9BeP2h7n0taVtQHrhbGhuIe2E.svg)](https://asciinema.org/a/9BeP2h7n0taVtQHrhbGhuIe2E)
(the demo shows browsing the excellent [cursive](https://github.com/gyscos/cursive) crate upon which this program's UI is built ❤️)

rsbrowse runs `rustdoc` on your code and tells it to save the type info for everything it sees. It then presents it in an interactive text-mode viewer. This lets you browse the structure of the program from the compiler's view.

# Requirements

* cargo
* nightly rust toolchain (this is needed to use the currently unstable `--output-format=json` flag in rustdoc)
* optional but *highly recommended*: the rustdoc JSON for the standard library
  * you can install this using `rustup component add rust-docs-json --toolchain nightly`
  * TODO: actually consuming this is not yet implemented

# Usage

```
$ rsbrowse <cargo workspace root>
```

rsbrowse will start up with the left pane listing all the workspace's crates as well as its dependencies.

Use the up and down keys to move within a column, and left and right to jump between columns. As you move within a column, columns to the right of it will be updated to show things inside of whatever you have selected.

At any time, you can press ENTER to bring up a dialog with info about whatever you have highlighted, including its source code. In this dialog, press TAB to switch to the buttons. The Debug button gives a dump of the raw rust-analysis data.

To exit, press ESC to activate the menu bar, and right arrow to select Quit.

# Help

rsbrowse is still pretty new and may have bugs. Unfortunately, as a curses application, panic text written to stderr gets lost. To capture it, redirect it, like `rsbrowse <path> 2>err.log` and try and reproduce what you did. (Also set `RUST_BACKTRACE=1` while you're at it.) Then please file an issue :)

To see a list of TODOs and ideas for future enhancements, see [`TODO.md`](TODO.md).
