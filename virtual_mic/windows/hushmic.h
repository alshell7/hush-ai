/*++
Hush AI Microphone additions to the SysVAD sample.

The Hush app writes enhanced mono audio at 48 kHz through a control device
(\\.\HushAIMicrophone). Each capture stream copies it from a kernel ring into its
WaveRT buffer. The reader is shared with the macOS driver (../ring.h): reads are indexed by
stream time, stay 10 ms behind the newest sample, resync when more than 100 ms off,
and output silence when the app is not writing. No locks, no waiting.
--*/
#pragma once

#define HUSH_MIC_RATE 48000
#define HUSH_MIC_CAPACITY 16384 /* frames, power of two */

#define HUSH_MIC_DEVICE_NAME L"\\Device\\HushAIMicrophone"
#define HUSH_MIC_LINK_NAME L"\\DosDevices\\HushAIMicrophone"
#define FILE_DEVICE_HUSH_MIC 0x8A75

/* Input: 0..4096 float samples (may be empty). Output: HUSH_MIC_POSITION. */
#define IOCTL_HUSH_MIC_WRITE CTL_CODE(FILE_DEVICE_HUSH_MIC, 0x800, METHOD_BUFFERED, FILE_WRITE_ACCESS)
#define HUSH_MIC_MAX_WRITE 4096

typedef struct _HUSH_MIC_POSITION
{
    ULONGLONG WritePosition; /* frames written by the app */
    ULONGLONG ReadPosition;  /* furthest frame read by any stream */
    ULONG ReadFrames;        /* size of the last stream read */
    ULONG Version;           /* HUSH_MIC_VERSION */
} HUSH_MIC_POSITION;

#define HUSH_MIC_VERSION 1

#ifdef _KERNEL_MODE

#include "../ring.h"

/* Per-stream reader state (rules in ring.h). Set synced = 0 when the stream starts. */
typedef HushRingReader HUSH_MIC_READER;

/* Copy audio for stream frames [Frame, Frame + Count) to Out. Any IRQL <= DISPATCH_LEVEL. */
VOID HushMicRead(_Inout_ HUSH_MIC_READER *Reader, _In_ ULONGLONG Frame, _Out_writes_(Count) float *Out, _In_ ULONG Count);

/* The control device lives with the adapter: create it in AddDevice, delete it on remove. */
NTSTATUS HushMicCreateControlDevice(_In_ PDRIVER_OBJECT DriverObject);
VOID HushMicDeleteControlDevice();

/* Route every request type once in DriverEntry, after PcInitializeAdapterDriver and the PnP
   hook. Requests for other device objects go to the saved handlers. */
VOID HushMicHookDispatch(_In_ PDRIVER_OBJECT DriverObject);

#endif
