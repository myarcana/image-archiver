write a Rust program in a file here that copies files (typically iPhone photos and videos) in a list of folders to a new location, generating normalized filenames with exiftool

program calling format:

```
collect_media <dirs...> -o <output_dir>
```

e.g.

```
collect_media /Volumes/Thumb/One /Volumes/Thumb/Two /Users/me/Pictures/2025  -o /Users/me/Pictures/My\ Library
```

Users must use one of `-o`, `--output-directory`,  or `--output-dir` exactly
once. All are synonyms for the output directory option. It can either be the
very first argument or the very last argument. If there are arguments before
and after it, the argument list is invalid.

When the function is run, perform these steps in order:

1. Validate the argument list
2. Validate that the input directories exist and be directories
3. Try to create the destination directory if it does not exist
4. Create a folder inside of destination directory called "Failed Cases"
5. Loop through all the image and video files in the `/Volumes/Thumb/One`,
   `/Volumes/Thumb/Two`,  `/Users/me/Pictures/2025` directories (do not recurse), generate
a normalized filename for them according to the rules outlined below, and then
copy the file to the output directory into a file with the new normalized
filename

Track progress so that on a second run, do not repeat files that have already been copied (and are still in the destination

# How to find image and video files?

Try to process every single file according to the below instructions. If there
is a file that can't be discovered a date for because none of the exiftool tags
exist in the file's metadata, or has another unexpected error, do not copy that file to the destination
directory. Instead:

1. put a soft link to the original file in the Failed Cases directory (subdirectory of the destination directory). If a file already exists in Soft Cases with the name, add a counter to the name to make a new soft link with a unique name.
2. make a file with the same name as the soft link next to the soft link with a .txt extension and include in it debug information about the error that caused this case to fail:
3. What is the filename and extension?
4. What are the file's creation time, access time, modified time?
5. What is the `file` mime type?
6. What are mdls `kMDItemContentTypeTree`, `kMDItemKind`?
7. what was the error that happened?



# Output file name format:

```
<Creation date> <Modified date> <counter>.<normalized original extension>
```

That is, the file creation date (formatting specified below), followed by a space, followed by the file modification date, followed by a space, followed by a base 10 numeral for collision resolution (starts at 1 by default, every filename should have a counter in it, it will only be greater than 1 for files with filenames that would otherwise collide), followed by a period, followed by the normalized original extension (normalization rules below)

# Date Formats

dates must be converted to UTC (no timezones) and then formatted as


YYYY-MM-DD_HH.mm.SS.NNN

That is, full numerical 4-digit year, 2-digit month (1-indexed, january is 01), 2-digit day (1-indexed)
no colons because colons are not allowed in filenames in some systems

24 hour clock hours, minutes, seconds, milliseconds

example:

2025-12-17_21.58.00.816

# How to choose the dates

Use exiftool's metadata tags. Select the first tag that is valid from the list of order of preference.

How to check validity:

1. the tag exists
2. the date it specifies is not in the future
3. the date it specifies is not the unix epoch, FILETIME epoch, OLE Automation / COM / Excel serial date, macOS (Classic) epoch (1904-01-01), macOS (modern) / iOS epoch (2001-01-01 00:00:00 UTC), NTP epoch (1900-01-01), GPS epoch (1980-01-06)
4. the date it specifies is nonzero

If it is valid, use it and perform the copy. If it is valid AND it is older than 2010, log a warning in the console.

First try exiftool fast. If none of the tags are found, then fallback to exiftool ExtractEmbedded.

Order of preference for creation date:

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

Order of preference for modified date:

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

When using one of these metadata tags, if there is more than one entry in the metadata for it (likely for Track and Media dates), use the mode (most common value). If there is a tie, use the earliest value.

Convert the found date to UTC. There are exiftool metadata tags that also specify timezones if the timezone is not in the string (for example Offset Time, Offset Time Original, Offset Time Digitized). If there is no timezone information, just assume it is already UTC.

# Normalization Rules for file extensions

All file extensions must be capitalized even if the input file's extension was not capitalized.

JPG and JPEG must both be normalized to JPG

# How to handle collisions

If a file with the generated name already exists in the destination, compare the files byte-by-byte. If the files are exactly the same, do not copy the file, just leave the old file there.

If the files are actually different, then keep incrementing the counter until there is no longer a file with the same filename in the destination directory, and copy the file to the calculated filename.

<example_exiftool_output_video>
ExifTool Version Number         : 13.36
File Name                       : GHYV2829.MOV
Directory                       : /Users/me/Pictures/iPhone Taiwan 2025-11-10
File Size                       : 3.3 MB
File Modification Date/Time     : 2025:08:10 10:15:12+08:00
File Access Date/Time           : 2025:11:29 09:28:50+08:00
File Inode Change Date/Time     : 2025:11:10 16:57:44+08:00
File Permissions                : -rw-------
File Type                       : MOV
File Type Extension             : mov
MIME Type                       : video/quicktime
Major Brand                     : Apple QuickTime (.MOV/QT)
Minor Version                   : 0.0.0
Compatible Brands               : qt
Media Data Size                 : 3332452
Media Data Offset               : 36
Movie Header Version            : 0
Create Date                     : 2025:08:10 03:43:16
Modify Date                     : 2025:08:10 03:43:16
Time Scale                      : 600
Duration                        : 1.77 s
Preferred Rate                  : 1
Preferred Volume                : 100.00%
Matrix Structure                : 1 0 0 0 1 0 0 0 1
Preview Time                    : 0 s
Preview Duration                : 0 s
Poster Time                     : 0 s
Selection Time                  : 0 s
Selection Duration              : 0 s
Current Time                    : 0 s
Next Track ID                   : 5
Track Header Version            : 0
Track Create Date               : 2025:08:10 03:43:16
Track Modify Date               : 2025:08:10 03:43:16
Track ID                        : 1
Track Duration                  : 1.77 s
Track Layer                     : 0
Track Volume                    : 0.00%
Matrix Structure                : 0 1 0 -1 0 0 2142 0 1
Image Width                     : 1814
Image Height                    : 1020
Clean Aperture Dimensions       : 1814x1020
Production Aperture Dimensions  : 1814x1020
Encoded Pixels Dimensions       : 1814x1020
Media Header Version            : 0
Media Create Date               : 2025:08:10 03:43:16
Media Modify Date               : 2025:08:10 03:43:16
Media Time Scale                : 600
Media Duration                  : 1.77 s
Media Language Code             : und
Handler Class                   : Media Handler
Handler Type                    : Video Track
Handler Vendor ID               : Apple
Handler Description             : Core Media Video
Graphics Mode                   : ditherCopy
Op Color                        : 32768 32768 32768
Handler Class                   : Data Handler
Handler Type                    : Alias Data
Handler Vendor ID               : Apple
Handler Description             : Core Media Data Handler
Compressor ID                   : hvc1
Source Image Width              : 1814
Source Image Height             : 1020
X Resolution                    : 72
Y Resolution                    : 72
Compressor Name                 : HEVC
Bit Depth                       : 24
Video Frame Rate                : 28.868
Handler Type                    : Metadata Tags
Lens Model (eng-CN)             : iPhone 13 Pro Max back camera 5.7mm f/1.5
Focal Length In 35mm Format (eng-CN): 25
Track Header Version            : 0
Track Create Date               : 2025:08:10 03:43:16
Track Modify Date               : 2025:08:10 03:43:16
Track ID                        : 2
Track Duration                  : 1.73 s
Track Layer                     : 0
Track Volume                    : 100.00%
Matrix Structure                : 1 0 0 0 1 0 0 0 1
Media Header Version            : 0
Media Create Date               : 2025:08:10 03:43:16
Media Modify Date               : 2025:08:10 03:43:16
Media Time Scale                : 44100
Media Duration                  : 1.73 s
Media Language Code             : und
Handler Class                   : Media Handler
Handler Type                    : Audio Track
Handler Vendor ID               : Apple
Handler Description             : Core Media Audio
Balance                         : 0
Handler Class                   : Data Handler
Handler Type                    : Alias Data
Handler Vendor ID               : Apple
Handler Description             : Core Media Data Handler
Audio Format                    : lpcm
Audio Channels                  : 3
Audio Bits Per Sample           : 16
Audio Sample Rate               : 1
Track Header Version            : 0
Track Create Date               : 2025:08:10 03:43:16
Track Modify Date               : 2025:08:10 03:43:16
Track ID                        : 3
Track Duration                  : 1.73 s
Track Layer                     : 0
Track Volume                    : 0.00%
Matrix Structure                : 1 0 0 0 1 0 0 0 1
Media Header Version            : 0
Media Create Date               : 2025:08:10 03:43:16
Media Modify Date               : 2025:08:10 03:43:16
Media Time Scale                : 600
Media Duration                  : 1.73 s
Media Language Code             : und
Handler Class                   : Media Handler
Handler Type                    : NRT Metadata
Handler Vendor ID               : Apple
Handler Description             : Core Media Metadata
Gen Media Version               : 0
Gen Flags                       : 0 0 0
Gen Graphics Mode               : ditherCopy
Gen Op Color                    : 32768 32768 32768
Gen Balance                     : 0
Handler Class                   : Data Handler
Handler Type                    : Alias Data
Handler Vendor ID               : Apple
Handler Description             : Core Media Data Handler
Meta Format                     : mebx
Sample Time                     : 0 s
Sample Duration                 : 1.73 s
Video Orientation               : Rotate 90 CW
Track Header Version            : 0
Track Create Date               : 2025:08:10 03:43:16
Track Modify Date               : 2025:08:10 03:43:16
Track ID                        : 4
Track Duration                  : 1.44 s
Track Layer                     : 0
Track Volume                    : 0.00%
Matrix Structure                : 1 0 0 0 1 0 0 0 1
Media Header Version            : 0
Media Create Date               : 2025:08:10 03:43:16
Media Modify Date               : 2025:08:10 03:43:16
Media Time Scale                : 600
Media Duration                  : 0.00 s
Media Language Code             : und
Handler Class                   : Media Handler
Handler Type                    : NRT Metadata
Handler Vendor ID               : Apple
Handler Description             : Core Media Metadata
Gen Media Version               : 0
Gen Flags                       : 0 0 0
Gen Graphics Mode               : ditherCopy
Gen Op Color                    : 32768 32768 32768
Gen Balance                     : 0
Handler Class                   : Data Handler
Handler Type                    : Alias Data
Handler Vendor ID               : Apple
Handler Description             : Core Media Data Handler
Meta Format                     : mebx
Sample Time                     : 0 s
Sample Duration                 : 0.00 s
Still Image Time                : -1
Handler Type                    : Metadata Tags
Location Accuracy Horizontal    : 3.529401
Live Photo Auto                 : 1
Full Frame Rate Playback Intent : 1
Live Photo Vitality Score       : 0.939849615097046
Live Photo Vitality Scoring Version: 4
GPS Coordinates                 : 30 deg 25' 6.24" N, 119 deg 24' 13.32" E, 325.417 m Above Sea Level
Make                            : Apple
Model                           : iPhone 13 Pro Max
Software                        : 18.4
Creation Date                   : 2025:08:10 10:15:11+08:00
Content Identifier              : ABFAA2B5-94A2-4161-AC3B-A49393CAB20F
Lens Model                      : iPhone 13 Pro Max back camera 5.7mm f/1.5
Focal Length In 35mm Format     : 25
Image Size                      : 1814x1020
Megapixels                      : 1.9
Avg Bitrate                     : 15.1 Mbps
GPS Altitude                    : 325.417 m
GPS Altitude Ref                : Above Sea Level
GPS Latitude                    : 30 deg 25' 6.24" N
GPS Longitude                   : 119 deg 24' 13.32" E
Rotation                        : 90
GPS Position                    : 30 deg 25' 6.24" N, 119 deg 24' 13.32" E
Lens ID                         : iPhone 13 Pro Max back camera 5.7mm f/1.5
</example_exiftool_output_video>
<example_exiftool_output_image>
ExifTool Version Number         : 13.36
File Name                       : AEBJ1470.JPEG
Directory                       : /Users/me/Pictures/iPhone Taiwan 2025-11-10
File Size                       : 216 kB
File Modification Date/Time     : 2025:06:17 11:58:00+08:00
File Access Date/Time           : 2025:11:29 01:44:30+08:00
File Inode Change Date/Time     : 2025:11:10 16:55:52+08:00
File Permissions                : -rw-------
File Type                       : JPEG
File Type Extension             : jpg
MIME Type                       : image/jpeg
JFIF Version                    : 1.01
Resolution Unit                 : None
X Resolution                    : 72
Y Resolution                    : 72
Exif Byte Order                 : Big-endian (Motorola, MM)
Make                            : Apple
Camera Model Name               : iPhone 13
Orientation                     : Horizontal (normal)
X Resolution                    : 72
Y Resolution                    : 72
Resolution Unit                 : inches
Software                        : 18.5
Modify Date                     : 2025:06:17 11:58:00
Host Computer                   : iPhone 13
Exposure Time                   : 1/50
F Number                        : 1.6
Exposure Program                : Program AE
ISO                             : 40
Exif Version                    : 0232
Date/Time Original              : 2025:06:17 11:58:00
Create Date                     : 2025:06:17 11:58:00
Offset Time                     : +08:00
Offset Time Original            : +08:00
Offset Time Digitized           : +08:00
Shutter Speed Value             : 1/50
Aperture Value                  : 1.6
Brightness Value                : 3.721170105
Exposure Compensation           : 0
Metering Mode                   : Multi-segment
Flash                           : Off, Did not fire
Focal Length                    : 5.1 mm
Subject Area                    : 960 539 1056 475
Maker Note Version              : 15
Run Time Flags                  : Valid
Run Time Value                  : 365151273219750
Run Time Scale                  : 1000000000
Run Time Epoch                  : 0
AE Stable                       : Yes
AE Target                       : 176
AE Average                      : 179
AF Stable                       : Yes
Acceleration Vector             : 0.07993619143 -0.8648555877 -0.4764198955
Focus Distance Range            : 0.11 - 0.11 m
OIS Mode                        : 3
Image Capture Type              : ProRAW
Live Photo Video Index          : 1107296384
Photos App Feature Flags        : 0
AF Performance                  : 12 1 52
Signal To Noise Ratio           : 42.23209763
Photo Identifier                : 86596261-C710-451F-BA69-D742C2F5D753
Color Temperature               : 3455
Camera Type                     : Back Normal
Focus Position                  : 194
Semantic Style                  : {_0=1,_1=0,_2=0}
User Comment                    : {"isFromCamera":1,"longitude":"121.4078743831767","creatorBundleId":"dianping","orgFileModifiedDate":"2025:06:17 11:58:00","latitude":"31.235526948736705","orgWidth":0,"orgHeight":0}
Sub Sec Time Original           : 816
Sub Sec Time Digitized          : 816
Exif Image Width                : 1080
Exif Image Height               : 1440
Sensing Method                  : One-chip color area
Scene Type                      : Directly photographed
Custom Rendered                 : Custom
Exposure Mode                   : Auto
White Balance                   : Auto
Focal Length In 35mm Format     : 27 mm
Lens Info                       : 5.1mm f/1.6
Lens Make                       : Apple
Lens Model                      : iPhone 13 back camera 5.1mm f/1.6
Image Width                     : 1080
Image Height                    : 1440
Encoding Process                : Baseline DCT, Huffman coding
Bits Per Sample                 : 8
Color Components                : 3
Y Cb Cr Sub Sampling            : YCbCr4:2:0 (2 2)
Run Time Since Power Up         : 4 days 5:25:51
Aperture                        : 1.6
Image Size                      : 1080x1440
Megapixels                      : 1.6
Scale Factor To 35 mm Equivalent: 5.3
Shutter Speed                   : 1/50
Create Date                     : 2025:06:17 11:58:00.816+08:00
Date/Time Original              : 2025:06:17 11:58:00.816+08:00
Modify Date                     : 2025:06:17 11:58:00+08:00
Circle Of Confusion             : 0.006 mm
Field Of View                   : 67.4 deg
Focal Length                    : 5.1 mm (35 mm equivalent: 27.0 mm)
Hyperfocal Distance             : 2.86 m
Light Value                     : 8.3
Lens ID                         : iPhone 13 back camera 5.1mm f/1.6
</example_exiftool_output_image>

# IMPORTANT NOTE

The most important thing is that the script must not be destructive at all. There must be no risk whatsoever that the original files get corrupted or deleted.



