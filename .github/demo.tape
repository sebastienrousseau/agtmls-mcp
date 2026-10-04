# SPDX-FileCopyrightText: 2026 Sebastien Rousseau <sebastian.rousseau@gmail.com>
# SPDX-License-Identifier: Apache-2.0 OR MIT

Output .github/demo.gif

Set Shell "bash"
Set FontSize 15
Set Width 1200
Set Height 600
Set Padding 20
Set Theme "Catppuccin Mocha"
Set TypingSpeed 35ms

Hide
Type "export PS1='$ ' && export PATH=$PWD/target/release:$PATH && clear"
Enter
Show

Type "agtmls-mcp --version"
Enter
Sleep 2s

Type "agtmls-mcp --help"
Enter
Sleep 3s

Type "./examples/tools_list.sh"
Enter
Sleep 4s
