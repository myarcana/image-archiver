# Image Archiver (collect_media)

A Rust command-line tool that copies photos and videos (typically from iPhone) to a destination directory with normalized, date-based filenames extracted from EXIF metadata.

## Problem

Managing photos and videos from multiple sources (iPhone backups, external drives, various folders) is challenging because:

- Files have inconsistent naming conventions
- Duplicates are hard to identify
- Original creation/modification dates are often lost in filenames
- Files from different sources need to be consolidated into a single library

## Solution

`collect_media` consolidates media files from multiple input directories into a single output directory with:

- **Normalized filenames** based on EXIF metadata dates (creation and modification)
- **Duplicate detection** via byte-by-byte comparison
- **Non-destructive operation** - only copies files, never modifies or deletes originals
- **Failed case tracking** - problematic files are logged with debug information

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
