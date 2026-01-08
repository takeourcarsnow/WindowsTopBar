# Search Fix - Application Indexing

## Problem
The search was not finding installed applications from Program Files or Start Menu because:
1. The default search index paths only included the home directory
2. Program Files, Program Files (x86), and Start Menu directories were not being indexed

## Solution

### 1. Updated SearchConfig to Index Application Directories
**File: `src/config.rs`**

The `SearchConfig::default()` now includes:
- `ProgramFiles` environment variable directory
- `ProgramFiles(x86)` environment variable directory  
- User Start Menu directory (`AppData\Microsoft\Windows\Start Menu`)
- AppData Programs directory (`AppData\Local\Programs`)

### 2. Improved App Directory Detection
**File: `src/search.rs`**

Updated the `is_app_directory()` function to detect:
- `\program files\` and `\program files (x86)\`
- `\start menu\` and `\microsoft\windows\start menu\`
- `\appdata\local\programs\`
- `\appdata\roaming\` with .exe files
- `\common files\` and `\commonprogramfiles\`
- Portable app locations (`\app\` and `\application\`)

### 3. Better Logging
**File: `src/app.rs`**

Added verbose logging to show:
- Number of root directories being indexed
- Each root directory path
- This helps verify Program Files is actually being indexed

## How It Works Now

When you search for "Lightroom":
1. ✅ Program Files is indexed (contains Adobe Lightroom.exe)
2. ✅ The app detector marks it as high-priority
3. ✅ The relevance scorer gives it +1000 points (application boost)
4. ✅ Result: Adobe Lightroom.exe appears first

## Testing

To verify the fix is working:
1. Delete the cached search index: `~\AppData\Roaming\topbar\search_index_count.txt`
2. Restart the application
3. Open Quick Search (Alt+S by default)
4. Try searching for "Adobe", "Visual", "Lightroom", etc.
5. Applications from Program Files should appear first
