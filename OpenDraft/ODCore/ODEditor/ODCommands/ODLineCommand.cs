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
            var startPoint = await editor.GetPointAsync("Specify first point:");
            var endPoint = await editor.GetPointAsync("Specify second point:");
            var line = new ODLine(startPoint, endPoint);
            
            editor.DataManager.AddElement(line);
            editor.SetStatus("Line created successfully");
        }
    }
}