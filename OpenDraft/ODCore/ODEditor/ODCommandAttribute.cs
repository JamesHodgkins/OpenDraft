// ODCommandAttribute.cs
using System;

namespace OpenDraft.ODCore.ODEditor.ODCommands
{
    [AttributeUsage(AttributeTargets.Class, Inherited = false, AllowMultiple = false)]
    public class ODCommandAttribute : Attribute
    {
        public string Name { get; }
        public string Description { get; }
        public string[] Aliases { get; }

        public ODCommandAttribute(string name, string description = "", params string[] aliases)
        {
            Name = name;
            Description = description;
            Aliases = aliases;
        }
    }
}