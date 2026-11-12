$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type -AssemblyName System.Drawing

# What to do and the job's data come in as an environment variable and a JSON
# file, never as text in this script.
function Read-Job {
    Get-Content -LiteralPath $env:OPENDRAFT_PRINT_JOB -Raw -Encoding UTF8 | ConvertFrom-Json
}

switch ($env:OPENDRAFT_PRINT_MODE) {
    'list' {
        $default = (New-Object System.Drawing.Printing.PrinterSettings).PrinterName
        $items = @()
        foreach ($name in [System.Drawing.Printing.PrinterSettings]::InstalledPrinters) {
            $items += [pscustomobject]@{ name = [string]$name; default = ([string]$name -eq $default) }
        }
        ConvertTo-Json -InputObject $items -Compress
    }
    'caps' {
        $job = Read-Job
        $settings = New-Object System.Drawing.Printing.PrinterSettings
        $settings.PrinterName = [string]$job.printer
        if (-not $settings.IsValid) { throw 'no such printer' }
        $media = @()
        foreach ($paper in $settings.PaperSizes) {
            # Paper sizes are in hundredths of an inch.
            $media += [pscustomobject]@{
                name = [string]$paper.PaperName
                width = [math]::Round($paper.Width * 0.254, 1)
                height = [math]::Round($paper.Height * 0.254, 1)
            }
        }
        ConvertTo-Json -InputObject ([pscustomobject]@{ media = $media }) -Depth 4 -Compress
    }
    'print' {
        $job = Read-Job
        $image = [System.Drawing.Image]::FromFile([string]$job.png)
        try {
            $document = New-Object System.Drawing.Printing.PrintDocument
            $document.PrinterSettings.PrinterName = [string]$job.printer
            if (-not $document.PrinterSettings.IsValid) { throw 'no such printer' }
            $document.PrinterSettings.Copies = [int16]$job.copies
            $document.DocumentName = 'OpenDraft plot'
            $paper = $null
            foreach ($candidate in $document.PrinterSettings.PaperSizes) {
                if ([string]$candidate.PaperName -eq [string]$job.media) { $paper = $candidate }
            }
            if ($null -eq $paper) { throw 'no such paper' }
            $document.DefaultPageSettings.PaperSize = $paper
            $document.DefaultPageSettings.Landscape = [bool]$job.landscape
            $document.DefaultPageSettings.Margins = New-Object System.Drawing.Printing.Margins(0, 0, 0, 0)
            $document.PrintController = New-Object System.Drawing.Printing.StandardPrintController
            $width = [double]$job.width_mm / 25.4 * 100
            $height = [double]$job.height_mm / 25.4 * 100
            $document.add_PrintPage({
                param($sender, $e)
                # The page origin is the printable corner; the picture is the
                # whole sheet, so it starts at minus the hard margin.
                $x = -1 * $e.PageSettings.HardMarginX
                $y = -1 * $e.PageSettings.HardMarginY
                $e.Graphics.DrawImage($image, (New-Object System.Drawing.RectangleF($x, $y, $width, $height)))
                $e.HasMorePages = $false
            }.GetNewClosure())
            $document.Print()
        } finally {
            $image.Dispose()
        }
        'ok'
    }
    default { throw 'unknown mode' }
}
