using OpenDraft.ODCore.ODGeometry;
using System.Collections.Generic;

namespace OpenDraft.ODCore.ODData
{
    public class ODSymbolTable
    {
        private List<ODSymbolDefinition> SymbolTable = new List<ODSymbolDefinition>();

        public ODSymbolDefinition? GetSymbolDefinition(string symbolName)
        {
            foreach (ODSymbolDefinition symbol in SymbolTable)
            {
                if (symbol.Name == symbolName)
                    return symbol;
            }
            return null;
        }

        public void AddSymbol(ODSymbolDefinition symbol)
        {
            SymbolTable.Add(symbol);
        }
    }
}
