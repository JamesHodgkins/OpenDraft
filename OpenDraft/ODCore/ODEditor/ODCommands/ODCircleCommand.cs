// Example of using dynamic elements in a command
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODEditor.ODCommands;
using OpenDraft.ODCore.ODEditor.ODDynamics;
using OpenDraft.ODCore.ODGeometry;
using OpenDraft.ODCore.ODSystem;
using System.Threading.Tasks;
using System;

[ODCommand("CIRCLE", "Creates a circle", "C")]
public class ODCircleCommand : ODBaseEditorCommand
{
    public override async Task ExecuteAsync(IODEditorGateway editor)
    {
        ushort layId = editor.DataManager.LayerManager.GetActiveLayer();
        ODLayer lay = editor.DataManager.LayerManager.GetLayerByID(layId)!;

        ODPoint center = await editor.GetPointAsync("Specify center point:");

        ODRubberBandLine rubberBand = new ODRubberBandLine(center);
        rubberBand.LineWeight = ODSystem.GetRegistryValueAsDecimal("style/draw_tool_line_weight");
        rubberBand.Colour = new ODColour(ODSystem.GetRegistryValueAsString("style/draw_tool_colour") ?? "#FFFFFF");
        rubberBand.LineType = ODSystem.GetRegistryValueAsString("style/draw_tool_line_type");
        editor.AddDynamicElement(rubberBand);

        ODRubberBandCircle rubberCircle = new ODRubberBandCircle(center);
        rubberCircle.LineWeight = lay.LineWeight;
        rubberCircle.Colour = lay.Color;
        rubberCircle.LineType = lay.LineType;
        editor.AddDynamicElement(rubberCircle);


        ODPoint radPoint = await editor.GetPointAsync("Specify radius:");

        // TODO CHANGE POINTS TO VECTORS
        float radius = (float)Math.Sqrt(
            Math.Pow(radPoint.X - center.X, 2) + Math.Pow(radPoint.Y - center.Y, 2)
            );

        var circle = new ODCircle(center, radius);
        editor.DataManager.AddElement(circle); // Use DataManager directly

        editor.SetStatus("Circle created successfully");
    }
}