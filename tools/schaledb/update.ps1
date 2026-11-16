# Refreshes the SchaleDB snapshot Rio reads the students with: the students released in
# Japan, sorted by Id, each with the squad it fights in (Main: striker, Support: special),
# and their icons, the faces the game draws under a battle record's bars. The tests and the
# app read only this snapshot, never the network.
#   powershell -File tools/schaledb/update.ps1
$ErrorActionPreference = 'Stop'
$root = Join-Path $PSScriptRoot '..\..\data\schaledb'
$icons = Join-Path $root 'student-icons'
New-Item -ItemType Directory -Force $icons | Out-Null
Add-Type -AssemblyName System.Web.Extensions

# The response has no charset, which Windows PowerShell would read as Latin-1, and
# ConvertFrom-Json refuses keys that differ only in case, which students.json has.
$bytes = (Invoke-WebRequest -UseBasicParsing 'https://schaledb.com/data/jp/students.json').RawContentStream.ToArray()
$parser = New-Object System.Web.Script.Serialization.JavaScriptSerializer
$parser.MaxJsonLength = [int]::MaxValue
$students = $parser.DeserializeObject([Text.Encoding]::UTF8.GetString($bytes)).Values |
  Where-Object { $_.IsReleased[0] } |
  Sort-Object { [int64]$_.Id }

# One entry per line keeps the snapshot diffable when SchaleDB adds students.
$lines = foreach ($student in $students) {
  [ordered]@{
    id = [int64]$student.Id
    name = ($student.Name -replace '[\r\n]+', '')
    squad = $student.SquadType
  } | ConvertTo-Json -Compress
}
$text = "[`n" + ($lines -join ",`n") + "`n]`n"
[IO.File]::WriteAllText((Join-Path $root 'students.json'), $text, (New-Object Text.UTF8Encoding($false)))

$fetched = 0
foreach ($student in $students) {
  $path = Join-Path $icons "$($student.Id).webp"
  if (Test-Path $path) { continue }
  curl.exe -sSf -o $path "https://schaledb.com/images/student/icon/$($student.Id).webp"
  if ($LASTEXITCODE -ne 0) { throw "failed to fetch the icon of $($student.Id)" }
  # SchaleDB answers a path it does not have with its page, not an error.
  $head = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($path), 0, 4)
  if ($head -ne 'RIFF') {
    Remove-Item $path
    throw "the icon of $($student.Id) is no picture"
  }
  $fetched++
}
Write-Host "students: $($students.Count) entries, $fetched new icons"
