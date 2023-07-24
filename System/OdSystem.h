#ifndef OD_SY_SYSTEM_H
#define OD_SY_SYSTEM_H

/**************************************************************************************
* OpenDraft:    System Application Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdSystem.h                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         July 01, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   System class for managing setting variables and other related tasks.              *
*                                                                                     *
* Notes:                                                                              *
*   This class is a singleton, which is currently not thread safe.                    *
***************************************************************************************/


#include <unordered_map>						// Include unordered_map
#include <filesystem>							// Include filesystem
#include <variant>								// Include variant
#include <iostream>								// Include iostream
#include <Libraries/pugixml/src/pugixml.hpp>	// Include pugixml
#include "System/OdCore.h"						// Include Core Utilities


namespace OD
{
	namespace System
	{
		// System registry Variable Class
		class OdSystemVariable
		{
			std::string key;
			std::variant<bool, int, float, std::string> value;

			public:

				// Constructor
				OdSystemVariable(std::string key, std::variant<bool, int, float, std::string> value)
				{
					this->key = key;
					this->value = value;
				}

				// Get the key
				std::string getKey()
				{
					return key;
				}

				// Get the value
				std::variant<bool, int, float, std::string> getValue()
				{
					return value;
				}

				// Set the value
				void setValue(std::variant<bool, int, float, std::string> value)
				{
					this->value = value;
				}

				// Get type of value
				std::string getType()
				{
					return value.index() == 0 ? "bool" : value.index() == 1 ? "int" : value.index() == 2 ? "float" : "string";
				}
		};

		// Application Class
		class OdSystem
		{
			OdSystem(OdSystem& other) = delete;
			void operator=(const OdSystem&) = delete;

		protected:
			// Constructor
			OdSystem();

		private:

			// Static reference to the application instance
			static OdSystem* instance_;
			std::unordered_map<std::string, OdSystemVariable*> registry;	// Map of system variables

		public:

			// Get the application instance
			static OdSystem* getInstance();

			// Destructor
			~OdSystem();

			// Load system variables from file
			void loadRegistryVariablesFromFile();
			
			// Get system variable
			OdSystemVariable* getRegistryVariableByName(const char* name);

			// Set system variable
			void updateRegistryVariable(OdSystemVariable* aEntry);

			// Get draw line weight by index
			float getDrawLineWeightByIndex(int index);
			
		};

	}// namespace OD::System
} // namespace OD

#endif // !OD_SY_SYSTEM_H