using OpenDraft.ODCore.ODEditor.ODCommands;
using OpenDraft.ODCore.ODGeometry;
using System.Diagnostics;
using System.Threading.Tasks;

namespace OpenDraft.ODCore.ODEditor.ODCommands
{
    [ODCommand("LINE", "Creates a line", "L")]
    public class ODLineCommand : ODBaseEditorCommand
    {
        public override async Task ExecuteAsync(IODEditorGateway editor)
        {
            Debug.WriteLine("ODLineCommand starting...");

            var startPoint = await editor.GetPointAsync("Specify first point:");
            Debug.WriteLine($"Got first point: ({startPoint.X}, {startPoint.Y})");

            var endPoint = await editor.GetPointAsync("Specify second point:");
            Debug.WriteLine($"Got second point: ({endPoint.X}, {endPoint.Y})");

            Debug.WriteLine("Creating line element...");
            var line = new ODLine(startPoint, endPoint);
            editor.DataManager.AddElement(line);

            Debug.WriteLine("ODLineCommand completed successfully");
            editor.SetStatus("Line created successfully");

            // The context will be cleared in the finally block of RunCommandAsync
        }
    }
}