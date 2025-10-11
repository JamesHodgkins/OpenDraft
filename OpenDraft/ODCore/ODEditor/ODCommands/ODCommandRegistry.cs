// ODCommandRegistry.cs
using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;

namespace OpenDraft.ODCore.ODEditor.ODCommands
{
    public class ODCommandRegistry
    {
        private readonly Dictionary<string, Type> _commands = new Dictionary<string, Type>(StringComparer.OrdinalIgnoreCase);

        public void RegisterAssembly(Assembly assembly)
        {
            var commandTypes = assembly.GetTypes()
                .Where(t => typeof(IODEditorCommand).IsAssignableFrom(t) && !t.IsAbstract)
                .Where(t => t.GetCustomAttribute<ODCommandAttribute>() != null);

            foreach (var type in commandTypes)
            {
                var attribute = type.GetCustomAttribute<ODCommandAttribute>();
                if (attribute != null)
                {
                    _commands[attribute.Name] = type;

                    foreach (var alias in attribute.Aliases)
                    {
                        _commands[alias] = type;
                    }
                }
            }
        }

        public void RegisterCommand<T>(string name, params string[] aliases) where T : IODEditorCommand
        {
            _commands[name] = typeof(T);
            foreach (var alias in aliases)
            {
                _commands[alias] = typeof(T);
            }
        }

        public IODEditorCommand? CreateCommand(string commandName)
        {
            if (_commands.TryGetValue(commandName, out var commandType))
            {
                return (IODEditorCommand)Activator.CreateInstance(commandType);
            }
            return null;
        }

        public IEnumerable<string> GetRegisteredCommandNames()
        {
            return _commands.Keys.Distinct();
        }

        public string GetCommandDescription(string commandName)
        {
            if (_commands.TryGetValue(commandName, out var type))
            {
                var attribute = type.GetCustomAttribute<ODCommandAttribute>();
                return attribute?.Description ?? string.Empty;
            }
            return string.Empty;
        }
    }
}