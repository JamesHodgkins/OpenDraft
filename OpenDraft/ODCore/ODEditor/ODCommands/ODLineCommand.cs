// Example of using dynamic elements in a command
using OpenDraft.ODCore.ODEditor.ODCommands;
using OpenDraft.ODCore.ODEditor.ODDynamics;
using OpenDraft.ODCore.ODGeometry;
using System.Diagnostics;
using System.Threading.Tasks;

[ODCommand("LINE", "Creates a line", "L")]
public class ODLineCommand : ODBaseEditorCommand
{
    public override async Task ExecuteAsync(IODEditorGateway editor)
    {
        var vec = await editor.GetLineAsync("Specify start point:", "Specify end point:");

        var line = new ODLine(vec.start, vec.end);
        editor.DataManager.AddElement(line);
        editor.SetStatus("Line created successfully");
    }
}