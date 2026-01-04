# Image Archiver (collect_media)

A Rust command-line tool that archives photos and videos to a destination
directory with dates taken (best-guessed from metadata) as filenames. It tries
to avoid archiving more than one copy of the same photo.

## Problem

Photos.app sucks for backing up photos from iPhones because:
- It cannot copy photos off the iPhone in the first place (often it gets stuck on photos in the iPhone that have the same name in the filesystem, or gets stuck on other potentially-corrupted photos)
- It cannot resume partial backups (at least not for large backups)
- It cannot deal with backups of >1000 photos (which is a very common situation!)

And even after using Image Capture.app to get the iPhone's photos into MacOS in
the first place, you cannot use Photos.app to import and deduplicate the photos
and back the photos up to an external drive, because:
- daemons like mediaanalysisd, photoslibraryd, photoanalysisd will start up and
run forever, preventing the drives from ever being unmounted or ejected, at
least not at your convenience (even if you have the patience to wait a whole
day for a mere 2000 photos, you will never be able to eject the drive)
- it creates Apple duplicate files all over the place (`._*` files), making
junk on your external drive and then on your internal drive if you ever copy
everything back over

## Solution

This project consolidates media files from multiple input directories into a single output directory with:

- **Normalized filenames**: The filename will be a `YYYY-MM-DD_HH.mm.SS.NNN`
date timestamp, generated according to my heuristic that tries to narrow down
to the time that the item was created
- **Deduplication** via byte-by-byte comparison of the images (if the date timestamp matches)
- **Non-destructive operation** - never modifies or deletes original files if the destination and source volumes are different
- **Failed case tracking** - problematic files are logged with debug information, so the user can manually intervene

## Usage

```
collect_media <dirs...> -o <output_dir>
```

The output directory option (`-o`, `--output-dir`, or `--output-directory`) must appear either at the very beginning or the very end of the argument list.

### Examples

```bash
# Output option at the end
collect_media /Volumes/Thumb/One /Volumes/Thumb/Two ~/Pictures/2025 -o ~/Pictures/MyLibrary

# Output option at the beginning
collect_media -o ~/Pictures/MyLibrary /Volumes/Thumb/One /Volumes/Thumb/Two
```

## How It Works

### 1. Argument Validation
- Validates the argument list structure
- Ensures input directories exist and are directories
- Creates the output directory if it doesn't exist
- Creates a "Failed Cases" subdirectory for problematic files

### 2. File Processing
For each file in the input directories (non-recursive):

1. Extract EXIF metadata using `exiftool`
2. Determine creation and modification dates from metadata tags
3. Generate a normalized filename
4. Copy the file to the output directory

### 3. Date Extraction

Dates are extracted from EXIF metadata using a prioritized tag list. The first valid tag is used.

**Creation date priority:**
1. DateTimeOriginal
2. Media Create Date
3. CreateDate
4. Track Create Date
5. Creation Date
6. ModifyDate
7. Media Modify Date
8. UserComment.orgFileModifiedDate
9. Track Modify Date
10. File Modification Date/Time

**Modification date priority:**
1. ModifyDate
2. UserComment.orgFileModifiedDate
3. Media Modify Date
4. Track Modify Date
5. CreateDate
6. DateTimeOriginal
7. Track Create Date
8. Media Create Date
9. Creation Date
10. File Modification Date/Time

**Date validation rules:**
- Tag must exist
- Date must not be in the future
- Date must not be a known epoch (Unix, FILETIME, macOS, iOS, NTP, GPS, etc.)
- Dates before 2010 trigger a warning

### 4. Output Filename Format

```
<creation_date> <modified_date> <counter>.<EXTENSION>
```

**Date format:** `YYYY-MM-DD_HH.mm.SS.NNN` (UTC)

**Examples:**
```
2025-08-10_02.15.11.000 2025-08-10_02.15.12.000 1.MOV
2025-06-17_03.58.00.816 2025-06-17_03.58.00.000 1.JPG
```

**Extension normalization:**
- All extensions are uppercased
- `JPEG` is normalized to `JPG`

### 5. Collision Handling

When a file with the generated name already exists:

1. Compare files byte-by-byte
2. If identical: skip (file already archived)
3. If different: increment counter until a unique filename is found

### 6. Failed Cases

Files that cannot be processed (missing metadata, errors) are handled by:

1. Creating a symlink to the original file in the "Failed Cases" directory
2. Creating a `.txt` file with debug information:
   - Original filename and extension
   - File timestamps (creation, access, modified)
   - MIME type (from `file` command)
   - macOS metadata (`kMDItemContentTypeTree`, `kMDItemKind`)
   - The specific error that occurred

## Safety Guarantees

- **Read-only on source files** - Original files are never modified or deleted
- **Copy-only operations** - All files are copied, not moved
- **Resumable** - Progress is tracked to avoid reprocessing on subsequent runs

## Dependencies

- `exiftool` - For EXIF metadata extraction
- `file` - For MIME type detection (macOS/Linux built-in)
- `mdls` - For macOS metadata (macOS only)
