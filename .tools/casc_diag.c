// casc_diag.c —— 逐条探测 CascLib 各调用，定位无头读取失败的环节
#include <windows.h>
#include <stdio.h>
#include <string.h>
#include <wchar.h>

typedef void *PH;
typedef BOOL(__cdecl *PF_Open)(const wchar_t *, DWORD, PH *);
typedef BOOL(__cdecl *PF_OpenEx)(const wchar_t *, DWORD, DWORD, PH *);
typedef BOOL(__cdecl *PF_Close)(PH);
typedef PH  (__cdecl *PF_Find1)(PH, const wchar_t *, void *, void *);
typedef BOOL(__cdecl *PF_Find2)(PH, void *);
typedef BOOL(__cdecl *PF_FindEnd)(PH);
typedef PH  (__cdecl *PF_OpenFile)(PH, const wchar_t *, DWORD, DWORD);
typedef PH  (__cdecl *PF_OpenFile2)(PH, const wchar_t *, DWORD, DWORD, PH *);
typedef BOOL(__cdecl *PF_Read)(PH, void *, DWORD, LPDWORD);
typedef BOOL(__cdecl *PF_Size)(PH, PULONGLONG);
typedef BOOL(__cdecl *PF_Size32)(PH, PDWORD);
typedef BOOL(__cdecl *PF_CloseF)(PH);
typedef DWORD(__cdecl *PF_Err)(void);
typedef BOOL(__cdecl *PF_Locale)(PH, DWORD, PDWORD);
typedef DWORD(__cdecl *PF_StorageInfo)(PH, DWORD, void *, DWORD, PDWORD);

typedef struct { wchar_t n[260], p[260], t[260]; DWORD type, size; ULONGLONG ft; } FD;

int wmain(int argc, wchar_t **argv)
{
    const wchar_t *root = (argc > 1) ? argv[1] : L"D:\\Games\\d2r92777\\Diablo II Resurrected Infernal Edition";
    HMODULE m = LoadLibraryW(L"CascLib.dll");
    if (!m) { wprintf(L"LoadLibrary failed %lu\n", GetLastError()); return 1; }
    PF_Open   Open   = (PF_Open)  GetProcAddress(m, "CascOpenStorage");
    PF_OpenEx OpenEx = (PF_OpenEx)GetProcAddress(m, "CascOpenStorageEx");
    PF_Close  Close  = (PF_Close) GetProcAddress(m, "CascCloseStorage");
    PF_Find1  Find1  = (PF_Find1) GetProcAddress(m, "CascFindFirstFile");
    PF_Find2  Find2  = (PF_Find2) GetProcAddress(m, "CascFindNextFile");
    PF_FindEnd FindEnd = (PF_FindEnd)GetProcAddress(m, "CascFindClose");
    PF_OpenFile  OpenFile  = (PF_OpenFile) GetProcAddress(m, "CascOpenFile");
    PF_OpenFile2 OpenFile2 = (PF_OpenFile2)GetProcAddress(m, "CascOpenFile");
    PF_Read   Read   = (PF_Read)  GetProcAddress(m, "CascReadFile");
    PF_Size   Size   = (PF_Size)  GetProcAddress(m, "CascGetFileSize64");
    PF_Size32 Size32 = (PF_Size32)GetProcAddress(m, "CascGetFileSize");
    PF_CloseF CloseF = (PF_CloseF)GetProcAddress(m, "CascCloseFile");
    PF_Err    Err    = (PF_Err)   GetProcAddress(m, "GetCascError");
    PF_Locale Locale = (PF_Locale)GetProcAddress(m, "CascSetStorageLocale");
    PF_StorageInfo Info = (PF_StorageInfo)GetProcAddress(m, "CascGetStorageInfo");
    wprintf(L"CascLib loaded. Open=%p OpenEx=%p Find1=%p OpenFile=%p\n", Open, OpenEx, Find1, OpenFile);

    SetCurrentDirectoryW(root);
    PH h = NULL;
    BOOL ok = Open(root, 0, &h);
    wprintf(L"[1] CascOpenStorage('%ls') = %d (ret-as-handle=%p) handle=%p err=%lu\n", root, ok, (void *)ok, h, Err ? Err() : 0);

    if (Locale) { DWORD old = 0; BOOL r = Locale(h, 0, &old); wprintf(L"[2] CascSetStorageLocale(0) = %d oldMask=0x%x\n", r, old); }
    if (Info) {
        DWORD need = 0; BYTE buf[64]; memset(buf, 0, sizeof(buf));
        for (DWORD cls = 0; cls <= 5; ++cls) {
            need = 0;
            BOOL r = Info(h, cls, buf, sizeof(buf), &need);
            unsigned v = *(unsigned *)buf;
            wprintf(L"[3.%lu] CascGetStorageInfo(cls=%lu) = %d need=%lu u32=%u\n", cls, cls, r, need, v);
        }
    }

    FD fd; memset(&fd, 0, sizeof(fd));
    PH fh = Find1(h, L"*", &fd, NULL);
    wprintf(L"[4] CascFindFirstFile('*') = %p err=%lu name='%ls' size=%lu\n", fh, Err ? Err() : 0, fd.n, fd.size);

    fd.type = 0xFFFFFFFF;
    fh = Find1(h, L"*", &fd, NULL);
    wprintf(L"[5] FindFirstFile('*', type=0xFFFFFFFF) = %p err=%lu name='%ls'\n", fh, Err ? Err() : 0, fd.n);

    const wchar_t *cands[] = {
        L"local\\lng\\strings\\item-names.json",
        L"data\\local\\lng\\strings\\item-names.json",
        L"global\\excel\\itemstatcost.txt",
        L"data\\global\\excel\\itemstatcost.txt",
        L".build.info",
        L"Data\\data\\.build.info",
        NULL
    };
    for (int i = 0; cands[i]; ++i) {
        PH f = OpenFile(h, cands[i], 0, 0);
        DWORD e = Err ? Err() : 0;
        ULONGLONG sz = 0; DWORD sz32 = 0;
        if (f && Size) Size(f, &sz);
        if (f && Size32) Size32(f, &sz32);
        wprintf(L"[6.%d] CascOpenFile('%ls') = %p err=%lu size64=%llu size32=%lu\n", i, cands[i], f, e, sz, sz32);
        if (f && CloseF) CloseF(f);
    }

    if (fh && fh != INVALID_HANDLE_VALUE && FindEnd) FindEnd(fh);
    if (h && Close) Close(h);
    return 0;
}
