namespace Companion.Cwtools

open System
open System.Text
open System.Text.Json
open System.Runtime.InteropServices

[<UnmanagedFunctionPointer(CallingConvention.Cdecl)>]
type Prepare = delegate of nativeint * unativeint -> nativeint
[<UnmanagedFunctionPointer(CallingConvention.Cdecl)>]
type Free = delegate of nativeint -> unit
[<UnmanagedFunctionPointer(CallingConvention.Cdecl)>]
type Process = delegate of nativeint * unativeint * uint32 * uint32 -> nativeint

type InputEdit =
    { line:int; column:int; original_length:int; replacement_length:int; prefix_added:int; kind:string }
type PreparedInput =
    { source:string; edits:InputEdit array; recovery:JsonElement option }
    member this.OriginalLine(line:int) =
        match this.recovery with
        | Some evidence -> Math.Min(line,evidence.GetProperty("original_lines").GetInt32())
        | None -> line
    member this.OriginalColumn(line:int, column:int) =
        let mutable delta = 0
        let mutable mapped = None
        for edit in this.edits do
            if edit.line=line && edit.replacement_length<>edit.original_length && mapped.IsNone then
                let start = edit.column + delta
                if column < start then mapped <- Some(column-delta)
                elif column < start+edit.replacement_length then
                    mapped <- Some(edit.column + Math.Clamp(column-start-edit.prefix_added,0,edit.original_length))
                else delta <- delta + edit.replacement_length-edit.original_length
        mapped |> Option.defaultValue (column-delta)

type JominiInput(libraryPath:string) =
    let handle = NativeLibrary.Load libraryPath
    let prepare = Marshal.GetDelegateForFunctionPointer<Prepare>(NativeLibrary.GetExport(handle,"hoi4_cwtools_prepare"))
    let free = Marshal.GetDelegateForFunctionPointer<Free>(NativeLibrary.GetExport(handle,"hoi4_parser_free"))
    let parseDocument = Marshal.GetDelegateForFunctionPointer<Process>(NativeLibrary.GetExport(handle,"hoi4_parser_process"))
    member private _.Response(source:string, invoke:nativeint -> unativeint -> nativeint) =
        let bytes = Encoding.UTF8.GetBytes source
        let buffer = GCHandle.Alloc(bytes,GCHandleType.Pinned)
        try
            let result = invoke (buffer.AddrOfPinnedObject()) (unativeint bytes.Length)
            if result=0n then failwith "Jomini CWTools input preparation returned no result"
            try
                JsonDocument.Parse(Marshal.PtrToStringUTF8 result)
            finally free.Invoke(result)
        finally buffer.Free()
    member this.Prepare(source:string) =
        use doc = this.Response(source,fun ptr size -> prepare.Invoke(ptr,size))
        let root=doc.RootElement
        if root.TryGetProperty("error") |> fst then failwith (root.GetProperty("error").GetString())
        if root.GetProperty("parser_compatibility").GetInt32()<>1 then failwith "Invalid Jomini compatibility version"
        if root.GetProperty("reviewed_recovery").GetInt32()<>1 then failwith "Unsupported reviewed recovery version"
        let edits = root.GetProperty("edits").EnumerateArray() |> Seq.map (fun row ->
            let number name=row.GetProperty(name:string).GetInt32()
            { line=number "line"; column=number "column"; original_length=number "original_length"
              replacement_length=number "replacement_length"; prefix_added=number "prefix_added"
              kind=row.GetProperty("kind").GetString() }) |> Seq.toArray
        let recovery =
            match root.TryGetProperty("recovery") with
            | true,e when e.ValueKind=JsonValueKind.Object -> Some(e.Clone())
            | _ -> None
        {source=root.GetProperty("source").GetString();edits=edits;recovery=recovery}
    member this.Check(source:string) =
        use doc = this.Response(source,fun ptr size -> parseDocument.Invoke(ptr,size,1u,0u))
        match doc.RootElement.TryGetProperty("error") with
        | true,error -> error.GetString()
        | _ -> ""
    interface IDisposable with
        member _.Dispose() = NativeLibrary.Free handle

module InputClassification =
    let isDocumentation (relative:string) =
        let path=relative.Replace('\\','/').ToLowerInvariant()
        let name=IO.Path.GetFileName path
        path="interface/credits.txt" ||
        (path.StartsWith("gfx/fonts/") && name.EndsWith(".txt") &&
         ["ofl";"license";"copying";"readme"] |> List.exists (fun prefix -> name.StartsWith(prefix,StringComparison.Ordinal)))
