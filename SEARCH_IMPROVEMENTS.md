# Search Improvements - Topbar

## Problem
When searching for applications (e.g., "Lightroom"), the search was returning generic file matches instead of prioritizing installed programs. The results were ranked purely by filesystem order, not relevance.

## Solution
Implemented a **smart relevance ranking system** for search results with the following improvements:

### Changes Made

#### 1. **Added App Directory Tracking** (search.rs)
- Track which indexed files are from application directories:
  - `Program Files` and `Program Files (x86)`
  - `AppData` directories
  - User profiles
- Store these paths in a dedicated `app_paths` HashSet for O(1) lookup

#### 2. **Implemented Relevance Scoring Algorithm**
The new `calculate_relevance_score()` function ranks results by:

| Priority | Factor | Bonus | Details |
|----------|--------|-------|---------|
| **Highest** | App/Program Directory | +1000 | Files from Program Files, AppData, etc. |
| **High** | Exact Filename Match | +500 | Query matches full filename (without extension) |
| **High** | Match Ratio | +100× | Scores based on how much of filename matches |
| **Medium** | Executable Files | +50 | .exe, .lnk, .bat files preferred |
| **Medium** | Path Depth | -2× | Penalize deeply nested files |
| **Low** | Directory Proximity | +50/(depth+1) | Prefer files closer to root |

#### 3. **Updated Search Method**
- `search_prefix()` now collects all prefix matches, scores them, and returns sorted results
- Results are sorted by relevance score (descending), then alphabetically for stability

### Example: Searching for "Lightroom"

**Before:**
1. lightroom.txt (C:\Users\Documents\lightroom.txt)
2. lightroom_config.ini (C:\Windows\lightroom_config.ini)
3. Adobe Lightroom.exe (C:\Program Files\Adobe\Lightroom\Lightroom.exe)

**After:**
1. Adobe Lightroom.exe (C:\Program Files\Adobe\Lightroom\Lightroom.exe) ⭐
2. lightroom_config.ini (C:\Windows\lightroom_config.ini)
3. lightroom.txt (C:\Users\Documents\lightroom.txt)

### Benefits
✅ **Better UX**: Users find applications first  
✅ **Relevant Results**: Exact matches rank higher  
✅ **Smart Ordering**: Considers file type and location  
✅ **Performance**: Same O(n log n) complexity as before  

## Testing
The existing test suite continues to pass. The ranking system gracefully handles:
- Files without app-directory context (uses other factors)
- Mixed results (apps + documents)
- Executables vs text files
