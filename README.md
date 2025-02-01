# refine

[![Crates.io](https://img.shields.io/crates/v/refine.svg)](https://crates.io/crates/refine)
[![dependency status](https://deps.rs/repo/github/rsalmei/refine/status.svg)](https://deps.rs/repo/github/rsalmei/refine)
![Crates.io](https://img.shields.io/crates/d/refine)
![GitHub Sponsors](https://img.shields.io/github/sponsors/rsalmei)

### Refine your file collection using Rust!

## What it does

This tool will revolutionize the way you manage and organize your file collections! It offers a comprehensive set of features to help you find duplicated files based on both size and filename, seamlessly join them into a single directory with advanced conflict resolution, quickly list files from multiple directories sorted together by various criteria, effortlessly rename or strip filenames and directories using advanced regular expression rules, and even rebuild entire media collections by identifying groups of files with similar names and assigning a sequential number to each, allowing you to organize them in a way that makes sense to you.

> Use it to _refine_ your photo, music, movie, porn, etc. collections, with advanced features in a simple and efficient way!

I've made this tool to be the fastest and easiest way to organize media collections. I use it a lot, and I hope it can help you too. It will scan several given directories at once, and analyze all files and directories as a whole, performing some advanced operations on them.

And yes, it is blazingly fast, like all Rust 🦀 software!

Enjoy!

## How to use it

Install `refine` with:

```
cargo install refine
```

And that's it, you're ready to go! You can now call it anywhere.

## What's new

### New in 1.3

This version is mostly about polishing, with some improvements and bug fixes.

We have a smarter list command, which hides full paths by default and uses descending order for size and ascending for name and path; join: change no_remove flag to parents (n -> p) and some clash options; rebuild: change simple_match flag to simple and fix full mode, which was not resetting sequences; general polishing.

<details><summary>(previous)</summary>

### New in 1.2

Here is a much improved partial mode in Rebuild command, which can alter groups of filenames while preserving sequences, and even detect and fix gaps in sequences caused by deleted files.

### New in 1.1

Revamped join command!
It now supports non-empty target folders, and will resolve clashes accordingly.

Also, several enum CLI arguments now support aliases, and I've fixed join command still moving files even when copy was requested.

### New in 1.0

Yes, it is time. After a complete overhaul of the code, it's time to release 1.0!
<br>It's an accomplishment I'm proud of, which took over 70 commits and a month's work, resulting in most of the code being rewritten.
It is more mature, stable, and well-structured now.

The major motivation for this version is the rebuild Partial mode! We can now rebuild collections even when some directories are not available! This means that files not affected by the specified naming rules will stay the same, keeping their sequence numbers, while new files are appended after the highest sequence found. It is handy for collections on external drives or cloud storage which are not always connected, allowing you to, even on the go, rebuild new files without messing up previous ones.

And this also includes:

- rebuild: new `--replace` option to replace all occurrences of some string or regex in the filenames with another one.
- new internal CLI options handling, which enables commands to modify them prior to their execution.
    - the new rebuild partial mode is auto-enabled in case not all directories are currently available.

### New in 0.18

- rebuild: new force implementation that is easier to use
    - it conflicts with any other options so must be used alone
    - now it just overwrites filenames without exceptions → best used with `-i` or on already organized collections
    - improved memory usage

### New in 0.17

- join: new clash resolve option
    - by default, no changes are allowed in directories where clashes are detected
    - all directories with clashes are listed, showing exactly which files are in them

### New in 0.16

- complete overhaul of the scan system, allowing directories to be extracted alongside files
- new `join` command, already with directory support
- new magic `-i` and `-x` options that filter both files and directories
- new filter options for files, directories, and extensions
- rename: include full directory support

### New in 0.15

- nicer rename command output by parent directory
- new threaded yes/no prompt that can be aborted with CTRL-C

### New in 0.14

- rename: disallow by default changes in directories where clashes are detected
    - new `--clashes` option to allow them

### New in 0.13

- rename: new replace feature, finally!
- global: make strip rules also remove `.` and `_`, in addition to `-` and spaces
- global: include and exclude options do not check extensions
- dupes: remove case option, so everything is case-insensitive now

### New in 0.12

- global: new `--dir-in` and `--dir-out` options.

### New in 0.11

- new `rename` command
- rebuild, rename: improve strip exact, not removing more spaces than needed

### New in 0.10

- global: new `--exclude` option to exclude files

### New in 0.9

- new support for Ctrl-C, to abort all operations and gracefully exit the program at any time.
    - all commands will stop collecting files when Ctrl-C is pressed
    - both `dupes` and `list` command will show partial results
    - the `rebuild` command will just exit, as it needs all the files to run

### New in 0.8

- new "list" command

### New in 0.7

- global: new `--include` option to filter input files
- rebuild: new `--force` option to easily rename new files
- rebuild: new interactive mode by default, making `--dry_run` obsolete (removed), with new `--yes` option to bypass it (good for automation)
- rebuild: auto fix renaming errors
- dupes: faster performance by ignoring groups with one file (thus avoiding loading samples)
- rebuild: smaller memory consumption by caching file extensions

</details>

## Commands

All commands will:

1. scan all the given directories recursively (excluding hidden .folders)
    - can optionally perform only a shallow scan, or even filter files and directories based on regular expressions
2. load the metadata for each file, like size and creation date, required by some commands
3. execute the command and either print the results or guide the user to perform the changes
    - everything is always interactive, so you can review the changes before applying them

<details><summary>refine --help</summary>

```
Refine your file collection using Rust!

Usage: refine [OPTIONS] [DIRS]... <COMMAND>

Commands:
  dupes    Find possibly duplicated files by both size and filename
  join     Join files into the same directory
  list     List files from the given directories
  rebuild  Rebuild the filenames of media collections intelligently
  rename   Rename files in batch, according to the given rules
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version

Global:
  -i, --include <REGEX>  Include only these files and directories; checked without extension
  -x, --exclude <REGEX>  Exclude these files and directories; checked without extension
  -I, --dir-in <REGEX>   Include only these directories
  -X, --dir-ex <REGEX>   Exclude these directories
      --file-in <REGEX>  Include only these files; checked without extension
      --file-ex <REGEX>  Exclude these files; checked without extension
      --ext-in <REGEX>   Include only these extensions
      --ext-ex <REGEX>   Exclude these extensions
  -w, --shallow          Do not recurse into subdirectories
  [DIRS]...          Directories to scan

For more information, see https://github.com/rsalmei/refine
```

</details>

### The `dupes` command

The `dupes` command will analyze and report the possibly duplicated files, either by size or name. It will even load a sample from each file, to guarantee they are indeed duplicated. It is a small sample by default but can help reduce false positives a lot, and you can increase it if you want.

1. group all the files by size
2. for each group with the exact same value, load a sample of its files
3. compare the samples with each other and find possible duplicates
4. group all the files by words in their names
    - the word extractor ignores sequence numbers like file-1, file copy, file-3 copy 2, etc.
5. run 2. and 3. again, and print the results

<details><summary>refine dupes --help</summary>

```
Find possibly duplicated files by both size and filename

Usage: refine dupes [OPTIONS] [DIRS]...

Options:
  -s, --sample <BYTES>  Sample size in bytes (0 to disable) [default: 2048]
  -h, --help            Print help
```

</details>

Example:

```
$ refine dupes ~/Downloads /Volumes/External --sample 20480
```

### The `join` command

The `join` command will let you grab all files and directories in the given directories and join them into a single one. You can filter files however you like, and choose whether they will be joined by moving or copying. It will even remove the empty parent directories after a move joining!

> Note: any deletions are only performed after files and directories have been successfully moved/copied. So, in case any errors occur, the files and directories partially moved/copied will be found in the target directory, so you should manually delete them before trying again.

1. detect clashes, i.e. files with the same name in different directories, and apply the given clash strategy
2. detect already in-place files
3. print the resulting changes to the filenames and directories, and ask for confirmation
4. if the user confirms, apply the changes
5. remove any empty parent directories when moving files

<details><summary>refine join --help</summary>

```
Join files into the same directory

Usage: refine join [OPTIONS] [DIRS]...

Options:
  -t, --target <PATH>  The target directory; will be created if it doesn't exist [default: .]
  -b, --by <STR>       The type of join to perform [default: move] [possible values: move, copy]
  -c, --clashes <STR>  How to resolve clashes [default: sequence] [possible values: sequence, parent-name, name-parent, skip]
  -f, --force          Force joining already in place files and directories, i.e. in subdirectories of the target
  -p, --parents        Do not remove empty parent directories after joining files
  -y, --yes            Skip the confirmation prompt, useful for automation
  -h, --help           Print help
```

</details>

Example:

```
$ refine join ~/media/ /Volumes/External/ -i 'proj-01' -X 'ongoing' -t /Volumes/External/proj-01
```

### The `list` command

The `list` command will gather all the files in the given directories, sort them by name, size, or path, and display them in a friendly format.

1. sort all files by either name, size, or path
    - ascending by default for name and path, descending for size, or optionally reverse
2. print the results

<details><summary>refine list --help</summary>

```
List files from the given directories

Usage: refine list [OPTIONS] [DIRS]...

Options:
  -b, --by <STR>  Sort by [default: name] [possible values: name, size, path]
  -r, --rev       Reverse the default order (name:asc, size:desc, path:asc)
  -p, --paths     Show full file paths
  -h, --help      Print help
```

</details>

Example:

```
$ refine list ~/Downloads /Volumes/External --by size --desc
```

### The `rebuild` command

I’m really proud of the `rebuild` command. It smartly rebuilds all the filenames of entire media collections, e.g., musics by album/singer and videos by streamers and even photos from your camera. Sequence numbers are removed, filenames are stripped according to your needs, similar names are intelligently matched, groups are sorted deterministically by creation date, sequence numbers are regenerated, and files are finally renamed!

It's awesome to quickly find your collections neatly sorted automatically; it's like magic, getting all files cleaned up, sorted, and sequenced with a single command. And upon running it again, the tool will seem to recognize the new files that have been added, as it will regenerate everything but only display entries that need to be changed, as the rest are already correct! And in case you delete files, all the subsequent ones will be renamed accordingly! Quite impressive, don't you think?

And don't worry as this tool is interactive, so you can review all changes before applying them.

1. apply naming rules to strip or replace parts of the filenames
2. extract and strip sequence numbers from names
3. if force mode is enabled, set all names to the forced value
4. if smart match is enabled, remove spaces and underscores from names
5. group the files by their resulting names
6. sort the groups according to the files' created dates
7. regenerate sequence numbers for each group; if partial mode is enabled, continue from the highest sequence found in the group
   > Note that these groups can contain files from different directories, and it will just work
8. print the resulting changes to the filenames, and ask for confirmation
9. if the user confirms, apply the changes

<details><summary>refine rebuild --help</summary>

```
Rebuild the filenames of media collections intelligently

Usage: refine rebuild [OPTIONS] [DIRS]...

Options:
  -b, --strip-before <STR|REGEX>    Strip from the start of the filename; blanks nearby are automatically removed
  -a, --strip-after <STR|REGEX>     Strip to the end of the filename; blanks nearby are automatically removed
  -e, --strip-exact <STR|REGEX>     Strip all occurrences in the filename; blanks nearby are automatically removed
  -r, --replace <STR|REGEX=STR|$N>  Replace all occurrences in the filename with another; blanks are not touched
  -s, --simple                      Disable smart matching, so "foo bar.mp4", "FooBar.mp4" and "foo__bar.mp4" are different
  -f, --force <STR>                 Force to overwrite filenames (use the Global options to filter files)
  -p, --partial                     Assume not all directories are available, which retains current sequences (but fixes gaps)
  -y, --yes                         Skip the confirmation prompt, useful for automation
  -h, --help                        Print help
```

</details>

Example:

```
$ refine rebuild ~/media /Volumes/External -a 720p -a Bluray -b xpto -e old
```

### The `rename` command

The `rename` command will let you batch rename files like no other tool, seriously! You can quickly strip common prefixes, suffixes, and exact parts of the filenames, as well as apply any regex replacements you want. By default, in case a filename ends up clashing with other files in the same directory, that whole directory will be disallowed to make any changes. The list of clashes will be nicely formatted and printed, so you can manually check them. And you can optionally allow changes to other files in the same directory, removing only the clashes if you find it safe.

1. apply naming rules to strip or replace parts of the filenames
2. remove all changes from the whole directory where clashes are detected
    - optionally removes only the clashes, allowing other changes
3. print the resulting changes to the filenames and directories, and ask for confirmation
4. if the user confirms, apply the changes

<details><summary>refine rename --help</summary>

```
Rename files in batch, according to the given rules

Usage: refine rename [OPTIONS] [DIRS]...

Options:
  -b, --strip-before <STR|REGEX>    Strip from the start of the filename; blanks nearby are automatically removed
  -a, --strip-after <STR|REGEX>     Strip to the end of the filename; blanks nearby are automatically removed
  -e, --strip-exact <STR|REGEX>     Strip all occurrences in the filename; blanks nearby are automatically removed
  -r, --replace <STR|REGEX=STR|$N>  Replace all occurrences in the filename with another; blanks are not touched
  -c, --clashes                     Allow changes in directories where clashes are detected
  -y, --yes                         Skip the confirmation prompt, useful for automation
  -h, --help                        Print help
```

</details>

Example:

```
$ refine rename ~/media /Volumes/External -b "^\d+_" -r '([^\.]*?)\.=$1 '
```

## Changelog

<details><summary>(click to expand)</summary>

- 1.3.0 Jan 31, 2025: list: smarter list command, which hides full paths by default (with a flag for showing them if needed) and uses by default descending order for size and ascending for name and path (with a flag to reverse it if needed); join: change no_remove flag to parents (n -> p) and some clash options; rebuild: change simple_match flag to simple and fix full mode, which was not resetting sequences; general polishing.
- 1.2.1 Nov 19, 2024: just require newer regex, so deps badge won't show "maybe insecure".
- 1.2.0 Nov 19, 2024: rebuild: much improved partial mode which can alter groups of filenames while preserving sequences, and even detect and fix gaps in sequences caused by deleted files.
- 1.1.0 Oct 10, 2024: join: support not empty target folders and resolve clashes accordingly; include support for aliases in several enum CLI arguments; fix join by copy still moving files.
- 1.0.0 Oct 09, 2024: major overhaul; rebuild: new partial mode, new replace feature, auto-enable partial mode in case not all directories are available.
- 0.18.0 Aug 27, 2024: rebuild: new force implementation that is easier to use with improved memory usage.
- 0.17.1 Aug 15, 2024: global: fix `--shallow` option.
- 0.17.0 Aug 05, 2024: global: dedup input directories, enables to select only files by filtering extensions; join: new clash resolve option.
- 0.16.0 Ago 01, 2024: global: scan with directory support, new `join` command, new magic filter options, new filter options; rename: include full directory support.
- 0.15.0 Jul 18, 2024: rename: nicer command output by parent directory; new threaded yes/no prompt that can be aborted with CTRL-C.
- 0.14.0 Jul 11, 2024: rename: disallow by default changes in directories where clashes are detected, including new `--clashes` option to allow them.
- 0.13.0 Jul 10, 2024: rename: new replace feature; global: make strip rules also remove `.` and `_`, `--include` and `--exclude` options do not check file extensions; dupes: remove case sensitivity option.
- 0.12.0 Jul 09, 2024: global: new `--dir-in` and `--dir-out` options.
- 0.11.0 Jul 08, 2024: global: new `rename` command; rebuild, rename: improve strip exact.
- 0.10.0 Jul 02, 2024: global: new `--exclude`.
- 0.9.0 Jul 01, 2024: global: support for CTRL-C.
- 0.8.0 Jun 30, 2024: new `list` command.
- 0.7.1 Jun 28, 2024: global: `--include` is now case-insensitive; rebuild: fix smart detect not grouping some files, strip rules remove hyphens too.
- 0.7.0 Jun 27, 2024: global: new `--include`; rebuild: new `--force`, new interactive mode, new `--yes`, auto fix rename errors, smaller memory consumption; dupes: improved performance.
- 0.6.0 Jun 24, 2024: global: new `rebuild` command, general polishing overall.
- 0.5.0 Jun 20, 2024: support for shallow scan, verbose mode; dupes: ignores repetition systems.
- 0.4.0 Jun 17, 2024: include `dupes` command, support match case and changing sample size.
- 0.3.0 Nov 07, 2023: include dedup by both size and name.
- 0.2.2 Jun 04, 2022: use 2KB sample size.
- 0.2.1 Jun 04, 2022: improve error handling.
- 0.2.0 Jun 01, 2022: publish as `refine`, use split crate `human-repr`.
- 0.1.1 May 27, 2022: samples the center of the files, which seems to fix false positives.
- 0.1.0 May 25, 2022: first release, detects duplicated files, simple sampling strategy (1KB from the start of the files).

</details>

## License

This software is licensed under the MIT License. See the LICENSE file in the top distribution
directory for the full license text.


---
Maintaining an open source project is hard and time-consuming, and I've put much ❤️ and effort into
this.

If you've appreciated my work, you can back me up with a donation! Thank you. 😊

[<img align="right" src="https://cdn.buymeacoffee.com/buttons/default-orange.png" width="217px" height="51x">](https://www.buymeacoffee.com/rsalmei)
[<img align="right" alt="Donate with PayPal button" src="https://www.paypalobjects.com/en_US/i/btn/btn_donate_LG.gif">](https://www.paypal.com/donate?business=6SWSHEB5ZNS5N&no_recurring=0&item_name=I%27m+the+author+of+alive-progress%2C+clearly+and+about-time.+Thank+you+for+appreciating+my+work%21&currency_code=USD)

---
