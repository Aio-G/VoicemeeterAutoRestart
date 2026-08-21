// Implementação de referência em C (Win32 API pura)
#define UNICODE
#define _UNICODE

#include <windows.h>
#include <tlhelp32.h>
#include <wchar.h>

static const wchar_t *PROCESS_NAME = L"voicemeeter8x64.exe";
static const wchar_t *PROCESS_PATH  = L"C:\\Program Files (x86)\\VB\\Voicemeeter\\voicemeeter8x64.exe";
static const wchar_t *MUTEX_NAME    = L"Local\\VoicemeeterMonitorMutex";

BOOL IsProcessRunning(const wchar_t *processName) {
    HANDLE snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (snap == INVALID_HANDLE_VALUE) return FALSE;

    PROCESSENTRY32W pe;
    pe.dwSize = sizeof(pe);

    BOOL found = FALSE;

    if (Process32FirstW(snap, &pe)) {
        do {
            if (_wcsicmp(pe.szExeFile, processName) == 0) {
                found = TRUE;
                break;
            }
        } while (Process32NextW(snap, &pe));
    }

    CloseHandle(snap);
    return found;
}

HANDLE OpenProcessByName(const wchar_t *processName) {
    HANDLE snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (snap == INVALID_HANDLE_VALUE) return NULL;

    PROCESSENTRY32W pe;
    pe.dwSize = sizeof(pe);

    HANDLE hProc = NULL;

    if (Process32FirstW(snap, &pe)) {
        do {
            if (_wcsicmp(pe.szExeFile, processName) == 0) {
                hProc = OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pe.th32ProcessID);
                if (hProc) break;
            }
        } while (Process32NextW(snap, &pe));
    }

    CloseHandle(snap);
    return hProc;
}

BOOL StartProcess(const wchar_t *processPath) {
    wchar_t cmdLine[MAX_PATH * 2];
    wcsncpy(cmdLine, processPath, _countof(cmdLine) - 1);
    cmdLine[_countof(cmdLine) - 1] = L'\0';

    STARTUPINFOW si;
    PROCESS_INFORMATION pi;
    ZeroMemory(&si, sizeof(si));
    ZeroMemory(&pi, sizeof(pi));
    si.cb = sizeof(si);

    BOOL ok = CreateProcessW(
        NULL,
        cmdLine,
        NULL,
        NULL,
        FALSE,
        0,
        NULL,
        NULL,
        &si,
        &pi
    );

    if (ok) {
        CloseHandle(pi.hThread);
        CloseHandle(pi.hProcess);
    }

    return ok;
}

int main(void) {
    HWND hwndConsole = GetConsoleWindow();
    if (hwndConsole) {
        ShowWindow(hwndConsole, SW_HIDE);
    }

    HANDLE hMutex = CreateMutexW(NULL, TRUE, MUTEX_NAME);
    if (hMutex == NULL) {
        return 1;
    }

    if (GetLastError() == ERROR_ALREADY_EXISTS) {
        CloseHandle(hMutex);
        return 0;
    }

    if (!IsProcessRunning(PROCESS_NAME)) {
        StartProcess(PROCESS_PATH);
        Sleep(2000);
    }

    while (1) {
        HANDLE hProc = OpenProcessByName(PROCESS_NAME);

        if (hProc) {
            WaitForSingleObject(hProc, INFINITE);
            CloseHandle(hProc);
            Sleep(1000);
            StartProcess(PROCESS_PATH);
            Sleep(2000);
        } else {
            StartProcess(PROCESS_PATH);
            Sleep(3000);
        }
    }

    CloseHandle(hMutex);
    return 0;
}