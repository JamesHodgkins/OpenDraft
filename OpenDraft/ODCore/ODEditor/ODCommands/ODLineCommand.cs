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
        var startPoint = await editor.GetPointAsync("Specify first point:");

        // ADD THIS: Create crosshair at start point
        var crosshair = new ODCrosshairElement(startPoint);
        editor.AddDynamicElement(crosshair);

        var endPoint = await editor.GetPointAsync("Specify second point:");

        // ADD THIS: Clear dynamic elements when command completes
        editor.ClearDynamicElements();

        var line = new ODLine(startPoint, endPoint);
        editor.DataManager.AddElement(line);
        editor.SetStatus("Line created successfully");
    }
}