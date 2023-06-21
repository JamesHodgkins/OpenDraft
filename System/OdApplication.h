#ifndef OD_SY_APPLICATION_H
#define OD_SY_APPLICATION_H

/**************************************************************************************
* OpenDraft:    System Application Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdApplication.h                                                       *
* Contributors: James Hodgkins                                                        *
* Date:         June 10, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Root application class                                                            *
***************************************************************************************/


#include "System/OdCore.h"
#include "Graphics/OdGraphics.h"		// Include Graphics
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
			bool running = false;
			OdMainWindow* mainWindow = nullptr;
			OdResourceManager* resManager;

		public:

			OdApplication()
			{
				resManager = new OdResourceManager();
				mainWindow = new OdMainWindow(1280, 720, "OpenDraft");
				mainWindow->resourceManager = resManager;
				mainWindow->initialise();
				running = true;
			}


			bool isRunning()
			{
				return running;
			}


			OdMainWindow* getMainWindow()
			{
				return mainWindow;
			}


			void update()
			{
				if (!mainWindow->isRunning())
				{
					running = false;
					return;
				}

				mainWindow->triggerEventsChain();
				mainWindow->onFrame();
			}


			~OdApplication()
			{
				mainWindow->close();
				delete resManager;
				delete mainWindow;
			}

		};
	} // namespace System
}// namespace OD

#endif // OD_SY_APPLICATION_H