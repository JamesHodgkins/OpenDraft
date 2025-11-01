using OpenDraft.ODCore.ODEditor.ODCommands;
using OpenDraft.ODCore.ODMath;
using System.Threading.Tasks;

[ODCommand("SELECT", "Selects elements", "SEL")]
public class ODSelectCommand : ODBaseEditorCommand
{
    public override async Task ExecuteAsync(IODEditorGateway editor)
    {
        editor.SetStatus("Select elements - Click to select, Shift+Click to multi-select");

        // This command runs until cancelled
        while (!editor.CancellationToken.IsCancellationRequested)
        {
            try
            {
                // Wait for a point (mouse click)
                ODVec2 point = await editor.GetPointAsync("Select elements or press ESC to exit");

                // The actual selection logic is handled by the ODEditor.LeftMouseClicked method
                // We just wait for the next click
                editor.SetStatus("Select elements - Click to select, Shift+Click to multi-select");
            }
            catch (TaskCanceledException)
            {
                break;
            }
        }

        editor.SetStatus("Selection mode exited");
    }
}