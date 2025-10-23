using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODEditor.ODDynamics;
using OpenDraft.ODCore.ODGeometry;
using OpenDraft.ODCore.ODMath;
using System.Threading;
using System.Threading.Tasks;

namespace OpenDraft.ODCore.ODEditor.ODCommands
{
    public interface IODEditorGateway
    {
        ODDataManager DataManager { get; }

        Task<ODVec2> GetPointAsync(string prompt);
        Task<double> GetNumberAsync(string prompt);
        Task<string> GetTextAsync(string prompt);
        Task<string> GetChoiceAsync(string prompt, params string[] options);
        Task<(ODVec2 start, ODVec2 end)> GetLineAsync(string promptStart, string promptEnd);

        void SetStatus(string message);
        void ShowMessage(string message);

        void AddDynamicElement(ODDynamicElement element);
        void RemoveDynamicElement(ODDynamicElement element);
        void ClearDynamicElements();

        CancellationToken CancellationToken { get; }
    }
}