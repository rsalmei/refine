# files
### Files deduplication in Rust!

## What it does

This is a simple tool that will scan some given paths, and report the possibly duplicated files.

It is blazingly fast, since it is in Rust, and tiny.

In the future, it could make more than just detecting duplicates, like for example renaming files, hence the name just `files`.

## How it works

It will:
- recursively detect all files in the given paths (excluding hidden .folders and .files)
- sort all the files by their sizes
- for each group with the exact same size, a sample of each file will be retrieved and compared
- each same size/same sample group will be listed as a possible duplicate:

```
132.1 KB
/Users/you/Downloads/path/file.ext
/Users/you/Downloads/another-path/other.any
/Volumes/External/backup-path/back.001

248.6 MB
/Users/you/Downloads/video.mp4
/Volumes/External/backup-path/video.mpg.bak

...
```

- and finally, a brief receipt will be printed:
```
total files: 13512 (1567 duplicates)
```

## How to use it

```bash
❯ cargo run ~/Downloads /Volumes/Drive ...
```

Send as many sources as you want.

## Changelog
- 1.0.0 May 26, 2022: first release, detect duplicated files, fixed sampling strategy


## License
This software is licensed under the MIT License. See the LICENSE file in the top distribution directory for the full license text.


---
Maintaining an open source project is hard and time-consuming, and I've put much ❤️ and effort into this.

If you've appreciated my work, you can back me up with a donation! Thank you 😊

[<img align="right" src="https://cdn.buymeacoffee.com/buttons/default-orange.png" width="217px" height="51x">](https://www.buymeacoffee.com/rsalmei)
[<img align="right" alt="Donate with PayPal button" src="https://www.paypalobjects.com/en_US/i/btn/btn_donate_LG.gif">](https://www.paypal.com/donate?business=6SWSHEB5ZNS5N&no_recurring=0&item_name=I%27m+the+author+of+alive-progress%2C+clearly+and+about-time.+Thank+you+for+appreciating+my+work%21&currency_code=USD)

---
