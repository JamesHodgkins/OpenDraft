using System.Threading.Tasks;

namespace OpenDraft.ODCore.ODEditor.ODCommands
{
    public interface IODEditorCommand
    {
        string Name { get; }
        string Description { get; }
        Task ExecuteAsync(IODEditorGateway editor);
    }
}