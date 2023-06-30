#ifndef OD_SY_APPLICATION_H
#define OD_SY_APPLICATION_H

/**************************************************************************************
* OpenDraft:    System Application Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdApplication.h                                                       *
* Contributors: James Hodgkins                                                        *
* Date:         June 30, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Root application class                                                            *
*                                                                                     *
* Notes:                                                                              *
*   This class is a singleton, which is currently not thread safe.                    *
***************************************************************************************/



#include <iostream>
#include <Libraries/pugixml/src/pugixml.hpp>
#include "System/OdCore.h"			// Include Core Utilities
#include "Graphics/OdMainWindow.h"	// Include Main Window Class
#include "OdResourceManager.h"		// Include Resource Manager
#include "System/OdSystem.h"		// Include System Class


using namespace OD::Graphics;

namespace OD
{
	namespace System
	{
		// Application Class
		class OdApplication
		{
			OdApplication(OdApplication& other) = delete;
			void operator=(const OdApplication&) = delete;
		
		protected:
			// Constructor
			OdApplication();

		private:

			// Static reference to the application instance
			static OdApplication* instance_;

			bool running = false;					// Flag to indicate if the application is still running
			OdMainWindow* mainWindow = nullptr;		// Pointer to the main window
			OdResourceManager* resManager;			// Pointer to the resource manager
			OdSystem* system;						// Pointer to the system

		public:

			// Get the application instance
			static OdApplication* getInstance();

			// Destructor
			~OdApplication();

			// Check if the application is still running
			bool isRunning();

			// Update the application for the next frame
			void update();

			// Get the main window
			OdMainWindow* getMainWindow();
		};

	}// namespace OD::System
} // namespace OD

#endif // OD_SY_APPLICATION_H