// Example of using dynamic elements in a command
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODEditor.ODCommands;
using OpenDraft.ODCore.ODEditor.ODDynamics;
using OpenDraft.ODCore.ODGeometry;
using System.Threading.Tasks;

[ODCommand("LINE", "Creates a line", "L")]
public class ODLineCommand : ODBaseEditorCommand
{
    public override async Task ExecuteAsync(IODEditorGateway editor)
    {
        ODPoint start = await editor.GetPointAsync("Specify start point:");

        ushort layId = editor.DataManager.LayerManager.GetActiveLayer();
        ODLayer lay = editor.DataManager.LayerManager.GetLayerByID(layId)!;

        ODRubberBandLine rubberBand = new ODRubberBandLine(start);
        rubberBand.LineWeight = lay.LineWeight;
        rubberBand.Colour = lay.Color;
        rubberBand.LineType = lay.LineType;
        editor.AddDynamicElement(rubberBand);
        
        ODPoint end   = await editor.GetPointAsync("Specify end point:");

        var line = new ODLine(start, end);
        editor.DataManager.AddElement(line);
        editor.SetStatus("Line created successfully");
    }
}