# refine

### Refine your file collection using Rust!

## What it does

This is a tool that will help you manage your media collection. It will scan some given paths, and detect duplicated files, or even rebuild all their filenames according to your rules!

The `dupes` command will analyze and report the possibly duplicated files, either by size or name. It will even load a sample from each file, in order to guarantee they are indeed duplicated. It is a small sample by default but can help reduce false positives a lot, and you can increase it if you want.

The new `rebuild` command is a great achievement, if I say so myself. It will smartly rebuild the filenames of your entire collection by stripping parts of filenames you don't want, removing any sequence numbers, smartly detecting misspelled names by comparing with adjacent files, sorting the detected groups deterministically by creation date, regenerating sequence numbers, and finally renaming all these files! It's awesome to quickly find your video or music library neatly sorted automatically... And the next time you run it, since it is deterministic, it will only detect the new files added since the last time. Pretty cool, huh? And don't worry, you can review all the changes before applying them.

It is blazingly fast and tiny, made 100% in Rust 🦀!

In the future, this tool could make much more, like for instance moving duplicated files, renaming files without rebuilding everything, perhaps supporting aliases for names, including a GUI to enable easily acting upon files, etc., hence the open `refine` name...

## New in 0.7

- global: new --include option to filter input files
- rebuild: new --force option to easily rename new files
- rebuild: new interactive mode by default, making --dry_run obsolete (removed), with new --yes option to bypass it (good for automation)
- rebuild: auto fix renaming errors
- dupes: faster performance by ignoring groups with 1 file (thus avoiding loading samples)
- rebuild: smaller memory consumption by caching file extensions

## How to use it

Install with:

```
cargo install refine
```

Then just call it anywhere:

```bash
❯ refine dupes ~/Downloads /Volumes/Drive ...
```

Or:

```bash
❯ refine rebuild ~/Downloads /Volumes/Drive ...
```

Send as many sources as you want.

## How it works

<details> <summary>Command options</summary>

```
Refine your file collection using Rust!

Usage: refine [OPTIONS] [PATHS]... <COMMAND>

Commands:
  dupes    Find possibly duplicated files by both size and filename
  rebuild  Rebuild the filenames of collections of files intelligently
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version

Global:
  -i, --include <REGEX>  Include only some files; tested against filename+extension, case-insensitive
      --shallow          Do not recurse into subdirectories
  [PATHS]...         Paths to scan

For more information, see https://github.com/rsalmei/refine
```

</details>

### The `dupes` command

1. recursively detect all files in the given paths (excluding hidden .folders)
    - can optionally run only a shallow scan, or only include some of the files
2. sort all the files by their sizes and by their words
    - the word extractor ignores sequence numbers like file-1, file-2, file copy, etc.
3. for each group with the exact same value, a sample of each file will be retrieved and compared
4. each coincidence will be listed as possible duplicates

<details> <summary>Command options</summary>

```
Find possibly duplicated files by both size and filename

Usage: refine dupes [OPTIONS] [PATHS]...

Options:
  -s, --sample <BYTES>  Sample size in bytes (0 to disable) [default: 2048]
  -c, --case            Case-sensitive file name comparison
  -h, --help            Print help

Global:
  -i, --include <REGEX>  Include only some files; tested against filename+extension, case-insensitive
      --shallow          Do not recurse into subdirectories
  [PATHS]...         Paths to scan
```

</details>

Output:

```
-- by size

132.1kB x3
/Users/you/Downloads/path/file.ext
/Users/you/Downloads/another-path/other.any
/Volumes/External/backup-path/back.001

248.6MB x2
/Users/you/Downloads/video.mp4
/Volumes/External/backup-path/video.mpg.bak

-- by name

["bin", "cache", "query"] x2
904.2kB: ./target/debug/incremental/refine-1uzt8yoeb0t1e/s-gx7knsxvbx-1oc90bk-working/query-cache.bin
904.9kB: ./target/debug/incremental/refine-1uzt8yoeb0t1e/s-gx7knwsqka-w784iw-6s3nzkfcj1wxagnjubj1pm4v6/query-cache.bin

```

And, finally, a brief receipt will be printed:

```
total files: 13512
  by size: 339 duplicates
  by name: 12 duplicates
```

### The `rebuild` command

1. strip parts of the filenames, either before, or after, or exact some matches
2. remove any sequence numbers they might have, like "-3" or " copy 2"
3. smartly remove spaces and underscores, and detect misspelled names
4. group the resulting names accordingly
5. smartly choose the most likely correct name among the group
6. sort the entries according to their created date
7. regenerate a unified sequence with this ordering; <-- Note this occurs on the whole group, regardless of the directory the file resides!
8. renames the files to the new pattern, after your review and confirmation

<details> <summary>Command options</summary>

```
Rebuild the filenames of collections of files intelligently

Usage: refine rebuild [OPTIONS] [PATHS]...

Options:
  -b, --strip-before <STR|REGEX>  Remove from the start of the filename to this str; blanks are automatically removed
  -a, --strip-after <STR|REGEX>   Remove from this str to the end of the filename; blanks are automatically removed
  -e, --strip-exact <STR|REGEX>   Remove all occurrences of this str in the filename; blanks are automatically removed
  -s, --no-smart-detect           Detect and fix similar filenames (e.g. "foo bar.mp4" and "foo__bar.mp4")
  -f, --force <STR>               Easily set filenames for new files. BEWARE: use only on already organized collections
  -y, --yes                       Skip the confirmation prompt, useful for automation
  -h, --help                      Print help

Global:
  -i, --include <REGEX>  Include only some files; tested against filename+extension, case-insensitive
      --shallow          Do not recurse into subdirectories
  [PATHS]...         Paths to scan
```

</details>

Output:

```
/Users/you/Downloads/sketch.mp4 --> sketch-1.mp4
/Users/you/Downloads/video ok.mp4 --> video_ok-1.mp4               | note these three files, regardless of different
/Users/you/Downloads/path/video_ok-5.mp4 --> video_ok-2.mp4        | paths and different names, they were smarly
/Volumes/External/backup/Video_OK copy.mp4 --> video_ok-3.mp4      | detected and renamed as the same group!
/Users/you/Downloads/path/video not ok.mp4 --> video_not_ok-1.mp4
```

And, finally, a brief receipt will be printed, as well as the interactive prompt:

```
total files: 21126
  changes: 142
apply changes? [y|n]: _
```

## Changelog highlights

- 0.7.1 Jun 28, 2024: global: --include is now case-insensitive, rebuild: fix smart detect bug not grouping some files, rebuild: strip rules remove hyphens too.
- 0.7.0 Jun 27, 2024: global: new --include, rebuild: new --force, rebuild: new interactive mode, rebuild: new --yes, rebuild: auto fix rename errors, rebuild: smaller memory consumption, dupes: improved performance.
- 0.6.0 Jun 24, 2024: new `rebuild` command, general polishing overall.
- 0.5.0 Jun 20, 2024: support for shallow scan, verbose mode, dupes cmd ignores repetition systems.
- 0.4.0 Jun 17, 2024: include `dupes` command, support match case and changing sample size.
- 0.3.0 Nov 07, 2023: include dedup by both size and name.
- 0.2.2 Jun 04, 2022: use 2KB sample size.
- 0.2.1 Jun 04, 2022: improve error handling.
- 0.2.0 Jun 01, 2022: publish, use split crate `human-repr`.
- 0.1.1 May 27, 2022: samples the center of the files, which seems to fix false positives.
- 0.1.0 May 25, 2022: first release, detects duplicated files, simple sampling strategy (1KB from
  the start of the files).

## License

This software is licensed under the MIT License. See the LICENSE file in the top distribution
directory for the full license text.


---
Maintaining an open source project is hard and time-consuming, and I've put much ❤️ and effort into
this.

If you've appreciated my work, you can back me up with a donation! Thank you 😊

[<img align="right" src="https://cdn.buymeacoffee.com/buttons/default-orange.png" width="217px" height="51x">](https://www.buymeacoffee.com/rsalmei)
[<img align="right" alt="Donate with PayPal button" src="https://www.paypalobjects.com/en_US/i/btn/btn_donate_LG.gif">](https://www.paypal.com/donate?business=6SWSHEB5ZNS5N&no_recurring=0&item_name=I%27m+the+author+of+alive-progress%2C+clearly+and+about-time.+Thank+you+for+appreciating+my+work%21&currency_code=USD)

---
