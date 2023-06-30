#ifndef OD_SY_SYSTEM_H
#define OD_SY_SYSTEM_H

/**************************************************************************************
* OpenDraft:    System Application Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdSystem.h                                                       *
* Contributors: James Hodgkins                                                        *
* Date:         June 26, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Root application class                                                            *
*                                                                                     *
* Notes:                                                                              *
*   This class is a singleton, which is currently not thread safe.                    *
***************************************************************************************/


#include <unordered_map>						// Include unordered_map
#include <filesystem>
#include <iostream>								// Include iostream
#include <Libraries/pugixml/src/pugixml.hpp>	// Include pugixml
#include "System/OdCore.h"						// Include Core Utilities


namespace OD
{
	namespace System
	{
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
			std::unordered_map<std::string, int> systemVariables;	// Map of system variables

		public:

			// Get the application instance
			static OdSystem* getInstance();

			// Destructor
			~OdSystem();

			// Get system variable
			int getSystemVariable(const char* name);

			// Set system variable
			void setSystemVariable(const char* name, int value);

			// Load system variables from file
			void loadSystemVariables();
			
		};

	}// namespace OD::System
} // namespace OD

#endif // OD_SY_SYSTEM_H