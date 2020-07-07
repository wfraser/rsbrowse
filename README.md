rsbrowse
========

Browse Rust code from the compiler's perspective.

[![demo!](https://asciinema.org/a/9BeP2h7n0taVtQHrhbGhuIe2E.svg)](https://asciinema.org/a/9BeP2h7n0taVtQHrhbGhuIe2E)
(the demo shows browsing the excellent [cursive](https://github.com/gyscos/cursive) crate upon which this program's UI is built ❤️)

rsbrowse runs `rustc` on your code and tells it to save analysis info about what it sees. It then presents it in an interactive text-mode viewer. The analysis info is the same thing that powers RLS (Rust Language Server) for enhancing IDEs, but instead of using it to navigate source code, rsbrowse does the reverse and lets you browse the structure of the program from the compiler's view.

# Requirements

* cargo
* nightly rust compiler (this is needed to use the `-Z save-analysis` flag)
* optional but *highly recommended*: the save-analysis for the standard library
  * you can install this using `rustup component add rust-analysis`

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

To see a list of TODOs and ideas for future enhancements, see [`TODO.md`](TODO.md). Note that rust-analysis output is somewhat limited in what it gives us, so some things may be harder to implement than they seem.