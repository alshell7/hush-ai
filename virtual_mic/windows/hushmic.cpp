/*++
Hush AI Microphone: kernel ring, control device and stream reader. See hushmic.h.
--*/
#include <ntddk.h>
#include <wdmsec.h>
#include "hushmic.h"

/* Nonpaged: the reader runs in a timer DPC. */
static float g_Samples[HUSH_MIC_CAPACITY];
static volatile LONG64 g_WritePosition;
static volatile LONG64 g_ReadPosition;
static volatile LONG g_ReadFrames;
static PDEVICE_OBJECT g_ControlDevice;
/* PortCls handlers for every request type, saved before hooking. */
static PDRIVER_DISPATCH g_PortCls[IRP_MJ_MAXIMUM_FUNCTION + 1];

/* {6F4A3B0E-9C21-4D7A-9E55-3A1C2B8D4F10}: class of the control device object. */
static const GUID HushMicControlClass =
    {0x6f4a3b0e, 0x9c21, 0x4d7a, {0x9e, 0x55, 0x3a, 0x1c, 0x2b, 0x8d, 0x4f, 0x10}};

#pragma code_seg()
VOID HushMicRead(_Inout_ HUSH_MIC_READER *Reader, _In_ ULONGLONG Frame, _Out_writes_(Count) float *Out, _In_ ULONG Count)
{
    LONGLONG write = ReadAcquire64(&g_WritePosition);
    LONGLONG end = hush_ring_read(Reader, g_Samples, HUSH_MIC_CAPACITY, write, (LONGLONG)Frame, Out, Count);
    // Publish the furthest frame read by any stream.
    LONGLONG read = ReadAcquire64(&g_ReadPosition);
    while (end > read || read - end > HUSH_MIC_CAPACITY)
    {
        LONGLONG seen = InterlockedCompareExchange64(&g_ReadPosition, end, read);
        if (seen == read)
        {
            InterlockedExchange(&g_ReadFrames, (LONG)Count);
            break;
        }
        read = seen;
    }
}

static NTSTATUS Complete(PIRP Irp, NTSTATUS Status, ULONG_PTR Information)
{
    Irp->IoStatus.Status = Status;
    Irp->IoStatus.Information = Information;
    IoCompleteRequest(Irp, IO_NO_INCREMENT);
    return Status;
}

/* IOCTL_HUSH_MIC_WRITE at PASSIVE_LEVEL. One writer: the control device is exclusive. */
static NTSTATUS Write(PIRP Irp, PIO_STACK_LOCATION Stack)
{
    ULONG inLength = Stack->Parameters.DeviceIoControl.InputBufferLength;
    ULONG outLength = Stack->Parameters.DeviceIoControl.OutputBufferLength;
    if (inLength % sizeof(float) != 0 || inLength > HUSH_MIC_MAX_WRITE * sizeof(float) ||
        outLength < sizeof(HUSH_MIC_POSITION))
    {
        return Complete(Irp, STATUS_INVALID_PARAMETER, 0);
    }
    // METHOD_BUFFERED: input and output share the system buffer. Copy input first.
    // Copy the raw bits: no floating point here. The reader (ring.h) rejects NaN and clips.
    const UCHAR *samples = (const UCHAR *)Irp->AssociatedIrp.SystemBuffer;
    ULONG count = inLength / sizeof(float);
    LONGLONG write = ReadAcquire64(&g_WritePosition);
    ULONG first = (ULONG)(write & (HUSH_MIC_CAPACITY - 1));
    ULONG head = min(count, HUSH_MIC_CAPACITY - first);
    RtlCopyMemory(&g_Samples[first], samples, head * sizeof(float));
    RtlCopyMemory(&g_Samples[0], samples + head * sizeof(float), (count - head) * sizeof(float));
    WriteRelease64(&g_WritePosition, write + count);
    HUSH_MIC_POSITION *position = (HUSH_MIC_POSITION *)Irp->AssociatedIrp.SystemBuffer;
    position->WritePosition = (ULONGLONG)(write + count);
    position->ReadPosition = (ULONGLONG)ReadAcquire64(&g_ReadPosition);
    position->ReadFrames = (ULONG)ReadNoFence(&g_ReadFrames);
    position->Version = HUSH_MIC_VERSION;
    return Complete(Irp, STATUS_SUCCESS, sizeof(HUSH_MIC_POSITION));
}

/* Every request goes through here. The control device is recognized by its type, not by
   g_ControlDevice, so requests on a handle that outlives the device are still ours. */
_Dispatch_type_(IRP_MJ_CREATE)
_Dispatch_type_(IRP_MJ_CLEANUP)
_Dispatch_type_(IRP_MJ_CLOSE)
_Dispatch_type_(IRP_MJ_DEVICE_CONTROL)
_Dispatch_type_(IRP_MJ_PNP)
_Dispatch_type_(IRP_MJ_POWER)
_Dispatch_type_(IRP_MJ_SYSTEM_CONTROL)
static DRIVER_DISPATCH DispatchAny;
static NTSTATUS DispatchAny(PDEVICE_OBJECT DeviceObject, PIRP Irp)
{
    PIO_STACK_LOCATION stack = IoGetCurrentIrpStackLocation(Irp);
    if (DeviceObject->DeviceType != FILE_DEVICE_HUSH_MIC)
    {
        return g_PortCls[stack->MajorFunction](DeviceObject, Irp);
    }
    switch (stack->MajorFunction)
    {
    case IRP_MJ_CREATE:
    case IRP_MJ_CLEANUP:
    case IRP_MJ_CLOSE:
        return Complete(Irp, STATUS_SUCCESS, 0);
    case IRP_MJ_DEVICE_CONTROL:
        if (stack->Parameters.DeviceIoControl.IoControlCode == IOCTL_HUSH_MIC_WRITE)
        {
            return Write(Irp, stack);
        }
        break;
    }
    return Complete(Irp, STATUS_INVALID_DEVICE_REQUEST, 0);
}

#pragma code_seg("PAGE")
VOID HushMicHookDispatch(_In_ PDRIVER_OBJECT DriverObject)
{
    PAGED_CODE();
    // PortCls drivers hook MajorFunction the same way (SysVAD hooks IRP_MJ_PNP).
    for (ULONG i = 0; i <= IRP_MJ_MAXIMUM_FUNCTION; i++)
    {
#pragma warning(suppress: 28175)
        g_PortCls[i] = DriverObject->MajorFunction[i];
#pragma warning(suppress: 28175 28168 28169)
        DriverObject->MajorFunction[i] = DispatchAny;
    }
}

NTSTATUS HushMicCreateControlDevice(_In_ PDRIVER_OBJECT DriverObject)
{
    PAGED_CODE();
    if (g_ControlDevice) return STATUS_SUCCESS;
    UNICODE_STRING name = RTL_CONSTANT_STRING(HUSH_MIC_DEVICE_NAME);
    UNICODE_STRING link = RTL_CONSTANT_STRING(HUSH_MIC_LINK_NAME);
    // System and administrators: all access. Interactive users: read and write.
    UNICODE_STRING sddl = RTL_CONSTANT_STRING(L"D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)");
    PDEVICE_OBJECT device = NULL;
    NTSTATUS status = IoCreateDeviceSecure(DriverObject, 0, &name, FILE_DEVICE_HUSH_MIC,
                                           FILE_DEVICE_SECURE_OPEN, TRUE /* one handle */, &sddl,
                                           &HushMicControlClass, &device);
    if (!NT_SUCCESS(status)) return status;
    status = IoCreateSymbolicLink(&link, &name);
    if (!NT_SUCCESS(status))
    {
        IoDeleteDevice(device);
        return status;
    }
    device->Flags |= DO_BUFFERED_IO;
    device->Flags &= ~DO_DEVICE_INITIALIZING;
    g_ControlDevice = device;
    return STATUS_SUCCESS;
}

VOID HushMicDeleteControlDevice()
{
    PAGED_CODE();
    if (!g_ControlDevice) return;
    UNICODE_STRING link = RTL_CONSTANT_STRING(HUSH_MIC_LINK_NAME);
    IoDeleteSymbolicLink(&link);
    IoDeleteDevice(g_ControlDevice);
    g_ControlDevice = NULL;
}
#pragma code_seg()
