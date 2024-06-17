# refine

### Refine your file collection using Rust!

## What it does

This is a tool that will scan some given paths, and report the possibly duplicated files both by
size and by name.
It will also load some sample bytes from each file, in order to guarantee they are indeed similar.

It is blazingly fast and tiny. It does not try to change anything anywhere, it just reports what it
finds.

In the future, this could make more than just detecting duplicates, like for instance moving those
files, renaming them, it could have a GUI to enable easily acting upon them, etc., hence the name
just `refine`.

## How it works

It will:

- recursively detect all files in the given paths (excluding hidden .folders and .files)
- sort all the files by their sizes
- for each group with the exact same size, a sample of each file will be retrieved and compared
- each same size/same sample groups will be listed as possible duplicates:

```
-- by size

132.1kB x3
/Users/you/Downloads/path/file.ext
/Users/you/Downloads/another-path/other.any
/Volumes/External/backup-path/back.001

248.6MB x2
/Users/you/Downloads/video.mp4
/Volumes/External/backup-path/video.mpg.bak

...

-- by name

["bin", "cache", "query"] x2
904.2kB: ./target/debug/incremental/refine-1uzt8yoeb0t1e/s-gx7knsxvbx-1oc90bk-working/query-cache.bin
904.9kB: ./target/debug/incremental/refine-1uzt8yoeb0t1e/s-gx7knwsqka-w784iw-6s3nzkfcj1wxagnjubj1pm4v6/query-cache.bin

...
```

And, finally, a brief receipt will be printed:

```
total files: 13512
  by size: 339 duplicates
  by name: 12 duplicates
```

## How to use it

Install with `cargo install refine`, then just:

```bash
❯ refine dupes ~/Downloads /Volumes/Drive ...
```

Send as many sources as you want.

## Changelog

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
