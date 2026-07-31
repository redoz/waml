# Review commit range index

The former `review-*.diff` files contained only the literal PowerShell value
`System.Object[]`. They were not valid diffs and contained no recoverable
review findings. They are removed instead of inventing reviewer content.

Use the exact full-hash ranges below to reproduce each code delta with:

`git diff --binary <base>..<head>`

| Former artifact | Base | Head |
|---|---|---|
| `review-8de8c0e0..b7d51ac1.diff` | `8de8c0e050f888d958cee6e79634e78c5f4dfeca` | `b7d51ac17da924a293c81dd45217e4575399fd1a` |
| `review-b7d51ac..ad7a985.diff` | `b7d51ac17da924a293c81dd45217e4575399fd1a` | `ad7a9859ac119bddf4f2540b1f1aca3fac421a63` |
| `review-ad7a985..fb86eb7.diff` | `ad7a9859ac119bddf4f2540b1f1aca3fac421a63` | `fb86eb7fb4e22b742d0b70ba68c7f952e36c2804` |
| `review-fb86eb7..672752e4.diff` | `fb86eb7fb4e22b742d0b70ba68c7f952e36c2804` | `672752e419e253241a7795612d1d9f11c346a3c3` |
| `review-672752e4..0ada1892.diff` | `672752e419e253241a7795612d1d9f11c346a3c3` | `0ada18923989f032e72a9ae56eea5b8458acc1f7` |
| `review-0ada1892..f43f0670.diff` | `0ada18923989f032e72a9ae56eea5b8458acc1f7` | `f43f0670464f5d61337f507fe54d5d2acde5488c` |
| `review-f43f0670..26a52b6d.diff` | `f43f0670464f5d61337f507fe54d5d2acde5488c` | `26a52b6dfcd83c5d22699e043f8d00d05bc3ac53` |
| `review-26a52b6d..bfdb27f2.diff` | `26a52b6dfcd83c5d22699e043f8d00d05bc3ac53` | `bfdb27f2625affc2ea150227984004db8cb77d76` |
| `review-bfdb27f2..1ffcadf5.diff` | `bfdb27f2625affc2ea150227984004db8cb77d76` | `1ffcadf5c20ee7046dd98654ce584422f851237c` |

Review outcomes that remain available are recorded in `progress.md` and
`ledger.md`. This index proves only the reviewed code ranges.
