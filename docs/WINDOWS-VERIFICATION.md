# Windows verification checklist

The GDI printing path cannot be unit-tested — Win32 handles against a real
spooler cannot be mocked honestly. Run this list once on a Windows machine
before any release.

1. The printer list matches the Windows printer list, and the system default is
   preselected.
2. A single-page PDF prints, correctly oriented and scaled inside the margins.
3. A multi-page PDF prints in the correct page order.
4. Two copies produce two copies.
5. Duplex produces a double-sided document.
6. Mono mode on a colour printer produces greyscale output.
7. Duplex and colour controls are reported unsupported for a printer that lacks
   them (`printer_capabilities_cmd`).
8. A JPG and a PNG print through the same path.
9. Switching the printer off mid-queue holds the queue and consumes no attempt;
   switching it back on resumes within a minute with no lost jobs.
10. A corrupt PDF fails on its own after three attempts, lands in `failed/`, and
    the queue continues with the next job.
11. Killing the app mid-print leaves that job as `failed` with kind
    `interrupted` on the next start — it must never reprint by itself.
12. With SumatraPDF installed and configured, a PDF that pdfium refuses still
    prints; with it absent, the job fails with a clear message.
