#ifndef OD_SY_APPLICATION_H
#define OD_SY_APPLICATION_H

/**************************************************************************************
* OpenDraft:    System Application Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdApplication.h                                                       *
* Contributors: James Hodgkins                                                        *
* Date:         June 26, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Root application class                                                            *
***************************************************************************************/


#include "System/OdCore.h"			// Include Core Utilities
#include "Graphics/OdMainWindow.h"	// Include Main Window Class
#include "OdResourceManager.h"		// Include Resource Manager


using namespace OD::Graphics;

namespace OD
{
	namespace System
	{
		// Application Class
		class OdApplication
		{
		private:

			// Static reference to the application instance
			static OdApplication* instance;

			bool running = false;					// Flag to indicate if the application is still running
			OdMainWindow* mainWindow = nullptr;		// Pointer to the main window
			OdResourceManager* resManager;			// Pointer to the resource manager

		public:
			// Get the application instance
			static OdApplication* getInstance();

			// Constructor
			OdApplication();

			// Destructor
			~OdApplication();

			// Check if the application is still running
			bool isRunning();

			// Update the application for the next frame
			void update();

			OdMainWindow* getMainWindow();
		};

	}// namespace OD::System
} // namespace OD

#endif // OD_SY_APPLICATION_H