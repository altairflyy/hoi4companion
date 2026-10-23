param([string]$ImagePath,[switch]$Worker)
$ErrorActionPreference='Stop'
[Console]::OutputEncoding=New-Object System.Text.UTF8Encoding($false)
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$null=[Windows.Storage.StorageFile,Windows.Storage,ContentType=WindowsRuntime]
$null=[Windows.Graphics.Imaging.BitmapDecoder,Windows.Graphics.Imaging,ContentType=WindowsRuntime]
$null=[Windows.Media.Ocr.OcrEngine,Windows.Foundation,ContentType=WindowsRuntime]
$null=[Windows.Globalization.Language,Windows.Foundation,ContentType=WindowsRuntime]
$awaitMethod=[System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {$_.Name -eq 'AsTask' -and $_.IsGenericMethod -and $_.GetGenericArguments().Count -eq 1 -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1'} | Select-Object -First 1
function Await($operation,$type){$task=$awaitMethod.MakeGenericMethod($type).Invoke($null,@($operation));$task.GetAwaiter().GetResult()}
$engine=[Windows.Media.Ocr.OcrEngine]::TryCreateFromUserProfileLanguages()
if(-not $engine){throw 'Nessun motore OCR Windows disponibile: installa una lingua con riconoscimento testo.'}
function Read-Ocr([string]$path,[string]$png){
 if($png){
  $null=[Windows.Storage.Streams.InMemoryRandomAccessStream,Windows.Storage.Streams,ContentType=WindowsRuntime]
  $stream=New-Object Windows.Storage.Streams.InMemoryRandomAccessStream
  $writer=New-Object Windows.Storage.Streams.DataWriter($stream)
  try{$writer.WriteBytes([Convert]::FromBase64String($png));$null=Await ($writer.StoreAsync()) ([uint32]);$null=$writer.DetachStream()}finally{$writer.Dispose()}
  $stream.Seek(0)
 }else{
  $file=Await ([Windows.Storage.StorageFile]::GetFileFromPathAsync($path)) ([Windows.Storage.StorageFile])
  $stream=Await ($file.OpenReadAsync()) ([Windows.Storage.Streams.IRandomAccessStreamWithContentType])
 }
 try{
  $decoder=Await ([Windows.Graphics.Imaging.BitmapDecoder]::CreateAsync($stream)) ([Windows.Graphics.Imaging.BitmapDecoder])
  $bitmap=Await ($decoder.GetSoftwareBitmapAsync()) ([Windows.Graphics.Imaging.SoftwareBitmap])
  try{
   if($bitmap.PixelWidth -gt [Windows.Media.Ocr.OcrEngine]::MaxImageDimension -or $bitmap.PixelHeight -gt [Windows.Media.Ocr.OcrEngine]::MaxImageDimension){throw 'Immagine oltre il limite OCR'}
   $result=Await ($engine.RecognizeAsync($bitmap)) ([Windows.Media.Ocr.OcrResult])
   $lines=@(foreach($line in $result.Lines){$words=@(foreach($word in $line.Words){$r=$word.BoundingRect;@{text=$word.Text;x=$r.X;y=$r.Y;width=$r.Width;height=$r.Height}});@{text=$line.Text;words=$words}})
   return @{lines=$lines;language=$engine.RecognizerLanguage.LanguageTag;width=$bitmap.PixelWidth;height=$bitmap.PixelHeight}
  }finally{$bitmap.Dispose()}
 }finally{$stream.Dispose()}
}
if($Worker){while($null -ne ($inputLine=[Console]::ReadLine())){try{$request=$inputLine|ConvertFrom-Json;$answer=Read-Ocr $request.path $request.png;$answer.token=$request.token}catch{$answer=@{error=$_.Exception.Message;lines=@()}};[Console]::WriteLine(($answer|ConvertTo-Json -Depth 7 -Compress))}}
else{Read-Ocr $ImagePath | ConvertTo-Json -Depth 7 -Compress}
