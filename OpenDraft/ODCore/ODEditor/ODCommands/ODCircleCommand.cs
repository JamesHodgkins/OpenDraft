// ODCircleCommand.cs
using OpenDraft.ODCore.ODEditor.ODCommands;
using OpenDraft.ODCore.ODEditor.ODDynamics;
using OpenDraft.ODCore.ODGeometry;
using System;
using System.Threading.Tasks;

namespace OpenDraft.ODCore.ODEditor.ODCommands
{
    [ODCommand("CIRCLE", "Creates a circle", "C")]
    public class ODCircleCommand : ODBaseEditorCommand
    {
        public override async Task ExecuteAsync(IODEditorGateway editor)
        {
            ODPoint center = await editor.GetPointAsync("Specify center point:");

            ODRubberBandLine rubberBand = new ODRubberBandLine(center);
            ODRubberBandCircle rubberCircle = new ODRubberBandCircle(center);
            editor.AddDynamicElement(rubberBand);
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
}