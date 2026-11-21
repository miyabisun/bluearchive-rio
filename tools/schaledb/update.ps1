# Refreshes the SchaleDB snapshot Rio reads the students with: the students released in
# Japan, sorted by Id, each with the squad it fights in (Main: striker, Support: special),
# its class (role), attack type and armor type, and, where the name is not written in kana,
# how it is read; their icons, the faces the game draws under a battle record's bars; and
# the icons SchaleDB shows the classes and the types with. The tests and the app read only
# this snapshot, never the network.
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
  $entry = [ordered]@{
    id = [int64]$student.Id
    name = ($student.Name -replace '[\r\n]+', '')
    squad = $student.SquadType
    role = $student.TacticRole
    attack = $student.BulletType
    armor = $student.ArmorType
  }
  # A name with letters other than kana (the variant in brackets aside) cannot be found by
  # typing kana: SchaleDB's readings of the family and the personal name stand in for it.
  if (($entry.name -replace ([char]0xFF08 + '.*?' + [char]0xFF09), '') -match ('[^' + [char]0x3040 + '-' + [char]0x30FF + ']')) {
    $personal = if ($student.PersonalNameRuby) { $student.PersonalNameRuby } else { $student.PersonalName }
    $entry.reading = ($student.FamilyNameRuby + $personal) -replace '\s', ''
  }
  $entry | ConvertTo-Json -Compress
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

# The icons of the classes, and the one icon each of the attack and the armor types (SchaleDB
# tells a type by the colour under it).
$ui = Join-Path $root 'ui'
New-Item -ItemType Directory -Force $ui | Out-Null
$marks = 'Role_Tanker', 'Role_DamageDealer', 'Role_Healer', 'Role_Supporter', 'Role_Vehicle', 'Type_Attack', 'Type_Defense'
foreach ($mark in $marks) {
  $path = Join-Path $ui "$mark.png"
  if (Test-Path $path) { continue }
  curl.exe -sSf -o $path "https://schaledb.com/images/ui/$mark.png"
  if ($LASTEXITCODE -ne 0) { throw "failed to fetch the icon $mark" }
  $head = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($path), 1, 3)
  if ($head -ne 'PNG') {
    Remove-Item $path
    throw "the icon $mark is no picture"
  }
  $fetched++
}
Write-Host "students: $($students.Count) entries, $fetched new icons"
