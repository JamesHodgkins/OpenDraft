#ifndef OD_GR_UI_WINDOW_H
#define OD_GR_UI_WINDOW_H

/**************************************************************************************
* OpenDraft:    GUI Window Class                                                      *
*-------------------------------------------------------------------------------------*
* Filename:     OdWindow.h                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         June 26, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A GUI base window class                                                           *
***************************************************************************************/

#include <vector>
#include <iostream>

#include <Windows.h>
#include <GL/glew.h>
#include <GLFW/glfw3.h>

#include "System/OdCore.h"
#include "Graphics/UI/OdComponent.h"
#include "Graphics/OdDraw.h"
#include "Graphics/UI/OdInput.h"

namespace OD
{
	namespace Graphics
	{
		class OdWindow : public OdComponent
		{
		protected:
			GLFWwindow* glfwHandle = nullptr;       // Handle to the GLFW window
			struct NVGcontext* context = nullptr;   // NanoVG context

			// Update properties following initialization or resize
			void updateProperties();

			// Load required resources
			void loadResources();


		public:

			GrInputMap input;                       // Input map for storing user input


			// Constructor
			OdWindow(int aWidth, int aHeight, const char* aTitle);

			// Set the window size
			void setSize(int aWidth, int aHeight);

			// Check if the window is still running
			bool isRunning();

			// Set the current context
			void makeCurrentContext();

			// Get the NanoVG context
			NVGcontext* getContext();

			// Returns 0,0 as window will always be at the root of the component tree
			OdVector2 getRelativeLocation();

			// Close the window and clean up resources
			void close();

			// Initialize the window and UI components
			virtual void initialise();

			// Render the window and UI components
			virtual void onFrame(NVGcontext* context) override;

			// Action window events
			void actionEvents(GrInputMap* aInput);

			// Process events for UI components
			void triggerEventsChain();
			void closeEvents();

			// Callback function for mouse movement
			static void mousePositionEventCallback(GLFWwindow* aWindow, double aPositionX, double aPositionY);

			// Callback function for window resize
			static void windowResizeCallback(GLFWwindow* aWindow, int aWidth, int aHeight);

			// Callback function for mouse clicks
			static void mouseClickEventCallback(GLFWwindow* aWindow, int aButton, int aAction, int aMods);

			// Callback function for mouse scroll
			static void mouseScrollEventCallback(GLFWwindow* aWindow, double aOffsetX, double aOffsetY);

			// Callback function for key events
			static void keyEventCallback(GLFWwindow* aWindow, int aKey, int aScancode, int aAction, int aMods);

		};

	} // namespace OD::Graphics
} // namespace OD

#endif // !OD_GR_UI_WINDOW_H
