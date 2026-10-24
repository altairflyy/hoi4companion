// Read-only semantic host. The engine and CWT DSL are upstream CWTools,
// not a translation of CWT into Companion-specific validation rules.
open System
open System.IO
open System.Text
open System.Text.Json
open System.Diagnostics
open CWTools.Common
open CWTools.Games
open CWTools.Games.Files
open CWTools.Games.HOI4
open Companion.Cwtools

let revision = "b377453dee803f9258be92cfc49896d09039702d"
let text (e: JsonElement) (key:string) = e.GetProperty(key).GetString()
let category code (message:string) =
    match code with
    | "CW001" | "CW100" | "CW225" | "CW226" | "CW234"
    | "CW254" | "CW255" | "CW256" | "CW257" | "CW258" | "CW259" | "CW260" | "CW266" | "CW268" -> "localisation"
    | "CW104" | "CW105" | "CW106" | "CW243" | "CW244" | "CW245" | "CW247" | "CW248" -> "scopes"
    | "CW112" | "CW113" | "CW222" | "CW261" -> "references"
    // Upstream uses CW240 for both primitive values and typed references.
    | "CW240" when message.StartsWith("Expected value of type ",StringComparison.Ordinal) -> "references"
    | _ -> "parameters"

let analyze (request: JsonElement) =
    if request.GetProperty("schema_version").GetInt32() <> 1 then failwith "Unsupported schema"
    let timer = Stopwatch.StartNew()
    let config = text request "rules_dir"
    let rules =
        Directory.EnumerateFiles(config, "*", SearchOption.AllDirectories)
        |> Seq.filter (fun f -> [".cwt"; ".csv"; ".log"] |> List.contains (Path.GetExtension f))
        |> Seq.sort
        |> Seq.map (fun f -> f, File.ReadAllText f)
        |> Seq.toList
    if rules.IsEmpty then failwith "No CWTools rules"
    // Upstream's rule loader logs and ignores malformed CWT. Reject that case
    // explicitly instead of reporting an apparently complete rules bundle.
    for filename,source in rules do
        if Path.GetExtension(filename)=".cwt" then
            match CWTools.Parser.CKParser.parseString source filename with
            | FParsec.CharParsers.ParserResult.Failure(message,_,_) -> failwith ("Invalid CWT: "+filename+": "+message)
            | _ -> ()
    // ZD supplies one already-merged effective installation. This preserves
    // launcher order and replace_path instead of CWTools' alphabetical mod order.
    let virtualRoot = "C:/__hoi4_companion_readonly__"
    use jomini = new JominiInput(text request "parser_library")
    let prepared = Collections.Generic.Dictionary<string,PreparedInput>()
    let sources = Collections.Generic.Dictionary<string,string>()
    let nonScripts = ResizeArray<string>()
    let files =
        request.GetProperty("files").EnumerateObject()
        |> Seq.choose (fun p ->
            let relative = p.Name.Replace('\\','/')
            if relative.StartsWith("/") || relative.Split('/') |> Array.exists (fun s -> s=".." || s.Contains(':')) then
                failwith "Invalid logical path"
            let original = p.Value.GetString()
            sources[relative] <- original
            if InputClassification.isDocumentation relative then
                nonScripts.Add relative
                None
            else
              let content =
                if [".txt";".gui";".gfx";".sfx";".asset";".map";".yml";".csv"] |> List.contains (Path.GetExtension relative) then
                    let bytes = File.ReadAllBytes original
                    if bytes.Length > 80000000 then failwith ("File exceeds 80 MB: " + original)
                    let decoded =
                        try UTF8Encoding(false,true).GetString(bytes).TrimStart('\uFEFF')
                        with :? DecoderFallbackException -> Encoding.GetEncoding(1252).GetString(bytes)
                    if [".yml";".csv"] |> List.contains (Path.GetExtension relative) then decoded
                    else
                        let input = jomini.Prepare decoded
                        if input.edits.Length>0 || input.recovery.IsSome then prepared[relative] <- input
                        input.source
                else ""
              Some(struct (virtualRoot + "/" + relative, content)))
        |> Seq.toArray
    let settings: HOI4Settings =
        { rootDirectories = [| ZD {path=virtualRoot; name=""; files=files} |]
          validation = {validateVanilla=true; experimental=false; langs=[| HOI4 HOI4Lang.English |]}
          rules = Some {ruleFiles=rules; validateRules=true; debugRulesOnly=false; debugMode=false}
          embedded = FromConfig(nonScripts |> Seq.map (fun path -> virtualRoot+"/"+path,"") |> Seq.toList,[])
          scriptFolders=None; excludeGlobPatterns=None; modFilter=None
          maxFileSize=Some 80; debugSettings=DebugSettings.Default }
    let game = HOI4Game(settings) :> IGame<HOI4ComputedData>
    let relative (file:string) = file.Replace('\\','/').Replace(virtualRoot+"/", "")
    let originalColumn file line column =
        match prepared.TryGetValue(relative file) with
        | true,input -> input.OriginalColumn(line,column)
        | _ -> column
    let originalLine file line =
        match prepared.TryGetValue(relative file) with
        | true,input -> input.OriginalLine line
        | _ -> line
    // LocalisationErrors consumes the rule-validation cache: the two calls
    // must be sequenced, not embedded in a list-expression evaluation.
    let validationErrors = game.ValidationErrors()
    let localisationErrors = game.LocalisationErrors(true,true)
    let errors =
        validationErrors @ localisationErrors
        |> List.map (fun e ->
            {| code=e.code; severity=e.severity.ToString().ToLowerInvariant()
               file=relative e.range.FileName; message=e.message
               line=originalLine e.range.FileName e.range.StartLine; column=originalColumn e.range.FileName e.range.StartLine e.range.StartColumn
               end_line=originalLine e.range.FileName e.range.EndLine; end_column=originalColumn e.range.FileName e.range.EndLine e.range.EndColumn
               category=category e.code e.message |})
        |> List.sortBy (fun e -> e.file,e.line,e.column,e.code,e.message)
    let parserErrors =
        game.ParserErrors() |> List.map (fun (file,message,pos) ->
            let bytes = File.ReadAllBytes sources[relative file]
            let decoded =
                try UTF8Encoding(false,true).GetString(bytes).TrimStart('\uFEFF')
                with :? DecoderFallbackException -> Encoding.GetEncoding(1252).GetString(bytes)
            let jominiError=jomini.Check decoded
            {| file=relative file; message=message; line=pos.Line; column=originalColumn file (int pos.Line) (int pos.Column-1)+1
               jomini_error=jominiError; classification=(if jominiError="" then "parser_disagreement" else "rejected_by_both")
               source_sha256=Convert.ToHexString(Security.Cryptography.SHA256.HashData bytes).ToLowerInvariant() |})
    let types = game.Types()
    let failedSources=parserErrors |> List.map (fun e->e.file) |> Set.ofList
    let counts = Collections.Generic.Dictionary<string,int>()
    let count key = counts[key] <- (match counts.TryGetValue key with true,n -> n | _ -> 0)+1
    for error in errors do
        count "total";count error.category;count error.severity
    {| schema_version=1; engine="cwtools"; engine_revision=revision
       parser_compatibility=1
       reviewed_recovery=1
       source_recoveries=prepared |> Seq.filter (fun pair -> not (failedSources.Contains pair.Key)) |> Seq.choose (fun pair -> pair.Value.recovery |> Option.map (fun evidence ->
           {| file=pair.Key; source_sha256=Convert.ToHexString(Security.Cryptography.SHA256.HashData(File.ReadAllBytes sources[pair.Key])).ToLowerInvariant(); evidence=evidence |})) |> Seq.sortBy _.file |> Seq.toArray
       input_adjustments=prepared |> Seq.filter (fun pair -> pair.Value.edits.Length>0) |> Seq.map (fun pair -> {| file=pair.Key; edits=pair.Value.edits |}) |> Seq.sortBy _.file |> Seq.toArray
       non_script_files=nonScripts |> Seq.sort |> Seq.toArray
       diagnostics=errors |> List.truncate 2000
       diagnostic_counts=counts; diagnostics_truncated=errors.Length>2000
       parser_errors=parserErrors
       documents=game.AllEntities() |> Seq.length
       entities=types |> Map.toSeq |> Seq.sumBy (snd >> Array.length)
       localisation_sources=game.AllLoadedLocalisation()
       type_names=types |> Map.toArray |> Array.map fst
       elapsed_ms=timer.ElapsedMilliseconds |}

[<EntryPoint>]
let main _ =
    Encoding.RegisterProvider(CodePagesEncodingProvider.Instance)
    // Keep stdout exclusively JSON, including if upstream writes diagnostics.
    let protocol = Console.Out
    Console.SetOut(Console.Error)
    try
        let input = Console.In.ReadToEnd()
        if input.Length > 16000000 then failwith "Request exceeds 16 MB"
        use document = JsonDocument.Parse(input)
        let result = analyze document.RootElement
        protocol.WriteLine(JsonSerializer.Serialize result)
        0
    with ex ->
        protocol.WriteLine(JsonSerializer.Serialize {|schema_version=1; error=ex.Message|})
        1
