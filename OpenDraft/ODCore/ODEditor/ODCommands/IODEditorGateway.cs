using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODGeometry;
using System.Threading;
using System.Threading.Tasks;

namespace OpenDraft.ODCore.ODEditor.ODCommands
{
    public interface IODEditorGateway
    {
        ODDataManager DataManager { get; }

        Task<ODPoint> GetPointAsync(string prompt);
        Task<double> GetNumberAsync(string prompt);
        Task<string> GetTextAsync(string prompt);
        Task<string> GetChoiceAsync(string prompt, params string[] options);

        void SetStatus(string message);
        void ShowMessage(string message);

        // Add cancellation token for long-running operations
        CancellationToken CancellationToken { get; }
    }
}