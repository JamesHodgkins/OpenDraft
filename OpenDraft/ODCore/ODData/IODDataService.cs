using Avalonia.Media;
using OpenDraft.ODCore.ODGeometry;

namespace OpenDraft.ODCore.ODData
{
    public interface IODDataService
    {
        // Layer access
        ODLayer? GetLayerByName(string name);
        ODLayer? GetLayerByID(ushort id);

        // Style conversion
        IDashStyle ToAvaloniaDashStyle(string dashStyle);

        // Symbol access
        ODSymbolDefinition? GetSymbolDefinition(string symbolName);
    }
}
