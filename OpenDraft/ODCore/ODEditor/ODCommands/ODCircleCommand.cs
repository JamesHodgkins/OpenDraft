// ODCircleCommand.cs
using OpenDraft.ODCore.ODEditor.ODCommands;
using OpenDraft.ODCore.ODGeometry;
using System.Threading.Tasks;

namespace OpenDraft.ODCore.ODEditor.ODCommands
{
    [ODCommand("CIRCLE", "Creates a circle", "C")]
    public class ODCircleCommand : ODBaseEditorCommand
    {
        public override async Task ExecuteAsync(IODEditorGateway editor)
        {
            var center = await editor.GetPointAsync("Specify center point:");
            var radius = await editor.GetNumberAsync("Specify radius:");

            var circle = new ODCircle(center, radius);
            editor.DataManager.AddElement(circle); // Use DataManager directly

            editor.SetStatus("Circle created successfully");
        }
    }
}