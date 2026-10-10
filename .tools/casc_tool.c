// casc_tool.c —— D2R CASC 无头提取/列举工具
//
// 依赖 CascLib.dll（D2RExtractor 自带，v1.50.0.206），与本程序放在同一目录。
// 编译（MSVC）:   cl /nologo /O2 /W3 /utf-8 casc_tool.c /link /out:casc_tool.exe
// 编译（MinGW）: x86_64-w64-mingw32-gcc -O2 -municode -o casc_tool.exe casc_tool.c
//
// 用法:
//   casc_tool.exe list    <storageDir> [mask]
//   casc_tool.exe extract <storageDir> <mask> <outDir>
//
// 例:
//   casc_tool.exe list    "D:\Games\...\Diablo II Resurrected Infernal Edition" "local\lng\strings\*.json"
//   casc_tool.exe extract "D:\Games\..." "global\excel\*.txt" "D:\out\excel"

#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>

typedef void *PHCASC;
typedef PHCASC (__cdecl *PFN_CascOpenStorage)(const wchar_t *, DWORD);
typedef BOOL   (__cdecl *PFN_CascCloseStorage)(PHCASC);
typedef PHCASC (__cdecl *PFN_CascFindFirstFile)(PHCASC, const wchar_t *, void *, void *);
typedef BOOL   (__cdecl *PFN_CascFindNextFile)(PHCASC, void *);
typedef BOOL   (__cdecl *PFN_CascFindClose)(PHCASC);
typedef PHCASC (__cdecl *PFN_CascOpenFile)(PHCASC, const wchar_t *, DWORD, DWORD);
typedef BOOL   (__cdecl *PFN_CascReadFile)(PHCASC, void *, DWORD, LPDWORD);
typedef BOOL   (__cdecl *PFN_CascGetFileSize64)(PHCASC, PULONGLONG);
typedef BOOL   (__cdecl *PFN_CascCloseFile)(PHCASC);
typedef DWORD  (__cdecl *PFN_GetCascError)(void);

static PFN_CascOpenStorage   pCascOpenStorage;
static PFN_CascCloseStorage  pCascCloseStorage;
static PFN_CascFindFirstFile pCascFindFirstFile;
static PFN_CascFindNextFile  pCascFindNextFile;
static PFN_CascFindClose     pCascFindClose;
static PFN_CascOpenFile      pCascOpenFile;
static PFN_CascReadFile      pCascReadFile;
static PFN_CascGetFileSize64 pCascGetFileSize64;
static PFN_CascCloseFile     pCascCloseFile;
static PFN_GetCascError      pGetCascError;

// CASC_FIND_DATA（1.50 布局：szFileName 在最前）
typedef struct _CASC_FIND_DATA {
    wchar_t   szFileName[260];
    wchar_t   szPlainName[260];
    wchar_t   szTypeName[260];
    DWORD     dwFileType;
    DWORD     dwFileSize;
    ULONGLONG tsFileTime;
} CASC_FIND_DATA;

static HMODULE g_dll = NULL;

static BOOL load_casc(const wchar_t *dllPath)
{
    g_dll = LoadLibraryW(dllPath);
    if (!g_dll) { fwprintf(stdout, L"[x] LoadLibrary failed: %ls (err=%lu)\n", dllPath, GetLastError()); return FALSE; }
    pCascOpenStorage   = (PFN_CascOpenStorage)  GetProcAddress(g_dll, "CascOpenStorage");
    pCascCloseStorage  = (PFN_CascCloseStorage) GetProcAddress(g_dll, "CascCloseStorage");
    pCascFindFirstFile = (PFN_CascFindFirstFile)GetProcAddress(g_dll, "CascFindFirstFile");
    pCascFindNextFile  = (PFN_CascFindNextFile) GetProcAddress(g_dll, "CascFindNextFile");
    pCascFindClose     = (PFN_CascFindClose)    GetProcAddress(g_dll, "CascFindClose");
    pCascOpenFile      = (PFN_CascOpenFile)     GetProcAddress(g_dll, "CascOpenFile");
    pCascReadFile      = (PFN_CascReadFile)     GetProcAddress(g_dll, "CascReadFile");
    pCascGetFileSize64 = (PFN_CascGetFileSize64)GetProcAddress(g_dll, "CascGetFileSize64");
    pCascCloseFile     = (PFN_CascCloseFile)    GetProcAddress(g_dll, "CascCloseFile");
    pGetCascError      = (PFN_GetCascError)     GetProcAddress(g_dll, "GetCascError");
    if (!pCascOpenStorage || !pCascFindFirstFile || !pCascOpenFile || !pCascReadFile) {
        fwprintf(stdout, L"[x] CascLib exports missing\n");
        return FALSE;
    }
    return TRUE;
}

static void make_dirs(const wchar_t *path)
{
    wchar_t buf[MAX_PATH * 4];
    wcsncpy(buf, path, MAX_PATH * 4 - 1); buf[MAX_PATH * 4 - 1] = 0;
    for (wchar_t *p = buf + 3; *p; ++p) {
        if (*p == L'\\' || *p == L'/') { wchar_t c = *p; *p = 0; CreateDirectoryW(buf, NULL); *p = c; }
    }
    CreateDirectoryW(buf, NULL);
}

static PHCASC open_storage(const wchar_t *root)
{
    // CascLib 依赖进程当前工作目录来定位 Data\data 等子目录（CascView 也是先切目录再打开）
    if (root && *root) SetCurrentDirectoryW(root);
    PHCASC h = pCascOpenStorage(root, 0);
    if (!h) wprintf(L"[x] CascOpenStorage failed: %ls (casc err=%lu, cwd fixed)\n", root, pGetCascError ? pGetCascError() : 0);
    return h;
}

static int cmd_list(const wchar_t *root, const wchar_t *mask)
{
    PHCASC h = open_storage(root);
    if (!h) return 1;
    CASC_FIND_DATA fd; memset(&fd, 0, sizeof(fd));
    PHCASC fh = pCascFindFirstFile(h, mask, &fd, NULL);
    if (!fh || fh == INVALID_HANDLE_VALUE) {
        fwprintf(stdout, L"[x] no match: %ls (casc err=%lu)\n", mask, pGetCascError ? pGetCascError() : 0);
        pCascCloseStorage(h); return 2;
    }
    unsigned long long n = 0, total = 0;
    do {
        fwprintf(stdout, L"%12lu  %ls\n", (unsigned long)fd.dwFileSize, fd.szFileName);
        total += fd.dwFileSize; ++n;
    } while (pCascFindNextFile(fh, &fd) && n < 200000);
    fwprintf(stdout, L"--- %llu files, %.2f MB\n", n, total / 1048576.0);
    pCascFindClose(fh);
    pCascCloseStorage(h);
    return 0;
}

static int extract_one(PHCASC h, const wchar_t *name, const wchar_t *outDir)
{
    PHCASC f = pCascOpenFile(h, name, 0, 0);
    if (!f) { fwprintf(stdout, L"  [x] cannot open %ls (err=%lu)\n", name, pGetCascError ? pGetCascError() : 0); return 1; }

    wchar_t path[MAX_PATH * 4];
    wcsncpy(path, outDir, MAX_PATH * 4 - 2);
    path[MAX_PATH * 4 - 2] = 0;
    size_t used = wcslen(path);
    if (used && path[used - 1] != L'\\') { path[used++] = L'\\'; path[used] = 0; }
    wcsncat(path, name, MAX_PATH * 4 - 2 - used);
    for (wchar_t *p = path; *p; ++p) if (*p == L'/') *p = L'\\';
    wchar_t *slash = wcsrchr(path, L'\\');
    if (slash) { *slash = 0; make_dirs(path); *slash = L'\\'; }

    FILE *fp = _wfopen(path, L"wb");
    if (!fp) { fwprintf(stdout, L"  [x] cannot write %ls\n", path); pCascCloseFile(f); return 1; }

    unsigned char *buf = (unsigned char *)malloc(1 << 20);
    ULONGLONG written = 0;
    for (;;) {
        DWORD got = 0;
        if (!pCascReadFile(f, buf, 1 << 20, &got) || got == 0) break;
        fwrite(buf, 1, got, fp);
        written += got;
    }
    free(buf); fclose(fp); pCascCloseFile(f);
    fwprintf(stdout, L"  ok  %8llu B  %ls\n", written, name);
    return 0;
}

static int cmd_extract(const wchar_t *root, const wchar_t *mask, const wchar_t *outDir)
{
    PHCASC h = open_storage(root);
    if (!h) return 1;
    CASC_FIND_DATA fd; memset(&fd, 0, sizeof(fd));
    PHCASC fh = pCascFindFirstFile(h, mask, &fd, NULL);
    if (!fh || fh == INVALID_HANDLE_VALUE) {
        fwprintf(stdout, L"[x] no match: %ls (casc err=%lu)\n", mask, pGetCascError ? pGetCascError() : 0);
        pCascCloseStorage(h); return 2;
    }
    int n = 0, bad = 0;
    do { if (extract_one(h, fd.szFileName, outDir) == 0) ++n; else ++bad; }
    while (pCascFindNextFile(fh, &fd) && (n + bad) < 200000);
    fwprintf(stdout, L"--- ok %d, failed %d\n", n, bad);
    pCascFindClose(fh);
    pCascCloseStorage(h);
    return bad ? 1 : 0;
}

int wmain(int argc, wchar_t **argv)
{
    if (argc < 3) {
        fwprintf(stdout, L"usage:\n  casc_tool.exe list    <storageDir> [mask]\n  casc_tool.exe extract <storageDir> <mask> <outDir>\n");
        return 2;
    }
    wchar_t dllPath[MAX_PATH * 4];
    GetModuleFileNameW(NULL, dllPath, MAX_PATH * 4);
    wchar_t *s = wcsrchr(dllPath, L'\\'); if (s) *(s + 1) = 0;
    wcscat(dllPath, L"CascLib.dll");
    if (!load_casc(dllPath)) return 3;

    if (!_wcsicmp(argv[1], L"list")) {
        const wchar_t *mask = (argc > 3) ? argv[3] : L"*";
        return cmd_list(argv[2], mask);
    }
    if (!_wcsicmp(argv[1], L"extract")) {
        if (argc < 5) { fwprintf(stdout, L"extract needs <storageDir> <mask> <outDir>\n"); return 2; }
        return cmd_extract(argv[2], argv[3], argv[4]);
    }
    fwprintf(stdout, L"unknown command: %ls\n", argv[1]);
    return 2;
}
