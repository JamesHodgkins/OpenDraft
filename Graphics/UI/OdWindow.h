#ifndef OD_GR_UI_WINDOW_H
#define OD_GR_UI_WINDOW_H

/**************************************************************************************
* OpenDraft:    GUI Window Class                                                      *
*-------------------------------------------------------------------------------------*
* Filename:     OdWindow.h                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         June 19, 2023                                                          *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A GUI base window class                                                           *
***************************************************************************************/



#include <vector>
#include <iostream>

#include <Windows.h>
#include <GL/glew.h>
#include <GLFW/glfw3.h>
#include <nanovg.h>

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
			GLFWwindow* glfwHandle = nullptr;		// Handle to the GLFW window
			struct NVGcontext* context = nullptr;	// NanoVG context
			GrInputMap input;						// Input map for storing user input

			void updateProperties()
			{
				int width = 0, height = 0;
				glfwGetWindowSize(glfwHandle, &width, &height);
				size.x = width;
				size.y = height;		
			}

		public:

			// Constructor
			OdWindow(int aWidth, int aHeight, const char* aTitle)
			{
				size = OdPoint(aWidth, aHeight);
				input = GrInputMap();

				// Initialize GLFW
				if (!glfwInit())
					return;

				// Create window with graphics context
				glfwHandle = glfwCreateWindow(size.x, size.y, aTitle, nullptr, nullptr);
				if (glfwHandle == nullptr)
					return;

				glfwMakeContextCurrent(glfwHandle);		// Set context as current
				glfwSwapInterval(1);					// Enable vsync
				glewInit();								// Initialize glew

				// Initialize NanoVG context (OpenGL backend)
				context = nvgCreateGL3(NVG_ANTIALIAS | NVG_STENCIL_STROKES | NVG_DEBUG);
				if (context == nullptr) {
					return;
				}

				// Bind this instance of GrWindow to the glfw window instance for static callbacks
				glfwSetWindowUserPointer(glfwHandle, this);

				// Set the callback function for mouse movement
				glfwSetCursorPosCallback(glfwHandle, mousePositionEventCallback);
				glfwSetMouseButtonCallback(glfwHandle, mouseClickEventCallback);
				glfwSetKeyCallback(glfwHandle, keyEventCallback);
				glfwSetWindowSizeCallback(glfwHandle, windowResizeCallback);

				// Update some properties following intitialisation
				updateProperties();
			}
	

			// Callback function for mouse movement
			static void mousePositionEventCallback(GLFWwindow* aWindow, double aPositionX, double aPositionY)
			{
				OdWindow* windowInstance = static_cast<OdWindow*>(glfwGetWindowUserPointer(aWindow));
				if (windowInstance) {
					// Access the instance and store the mouse position
					windowInstance->input.mouse.position.x = aPositionX;
					windowInstance->input.mouse.position.y = aPositionY;
				}
			}

			// Callback function for window resize
			static void windowResizeCallback(GLFWwindow* aWindow, int aWidth, int aHeight)
			{
				OdWindow* windowInstance = static_cast<OdWindow*>(glfwGetWindowUserPointer(aWindow));
				if (windowInstance) {
					// Access the instance and update size
					windowInstance->updateProperties();
				}
			}

			// Callback function for mouse clicks
			static void mouseClickEventCallback(GLFWwindow* aWindow, int aButton, int aAction, int aMods)
			{
				OdWindow* windowInstance = static_cast<OdWindow*>(glfwGetWindowUserPointer(aWindow));
				if (windowInstance)
				{
					switch (aButton)
					{
					case 0:
						if (aAction == GLFW_PRESS)
							windowInstance->input.mouse.leftButton.changeState(true);
						if (aAction == GLFW_RELEASE)
							windowInstance->input.mouse.leftButton.changeState(false);
						break;

					case 1:
						if (aAction == GLFW_PRESS)
							windowInstance->input.mouse.rightButton.changeState(true);
						if (aAction == GLFW_RELEASE)
							windowInstance->input.mouse.rightButton.changeState(false);
						break;

					case 2:
						if (aAction == GLFW_PRESS)
							windowInstance->input.mouse.middleButton.changeState(true);
						if (aAction == GLFW_RELEASE)
							windowInstance->input.mouse.middleButton.changeState(false);
						break;
					}
				}
			}

			// Callback function for key events
			static void keyEventCallback(GLFWwindow* aWindow, int aKey, int aScancode, int aAction, int aMods)
			{
				OdWindow* windowInstance = static_cast<OdWindow*>(glfwGetWindowUserPointer(aWindow));
				if (windowInstance) {
					GrInputMap* map = &windowInstance->input;

					if (map->keys.find(aKey) == map->keys.end())
						return;

					// Change button state to down
					if (aAction == GLFW_PRESS)
						map->keys[aKey].changeState(true);

					// Change button state to up
					if (aAction == GLFW_RELEASE)
						map->keys[aKey].changeState(false);

				}
			}

			// Set size (todo: set override when base equivalent is implemented)
			void setSize(int aWidth, int aHeight)
			{
				size.x = aWidth;
				size.y = aHeight;
				glfwSetWindowSize(glfwHandle, aWidth, aHeight);
			}

			// Check if the window is still running
			bool isRunning()
			{
				return !glfwWindowShouldClose(glfwHandle);
			}

			// Set the current context
			void makeCurrentContext()
			{
				glfwMakeContextCurrent(glfwHandle);
			}

			// Get the NanoVG context
			NVGcontext* getContext()
			{
				return context;
			}

			OdPoint getRelativeLocation()
			{
				return OdPoint(0, 0);
			}

			// Close the window and clean up resources
			void close()
			{
				// Destroy GLFW window and terminate GLFW
				glfwDestroyWindow(glfwHandle);
				glfwTerminate();

				// Delete UI components
				for (OdComponent* control : childComponents) {
					delete control;
				}
				childComponents.clear();

				// Delete NanoVG context
				nvgDeleteGL3(context);
			}

			// Initialize the window and UI components
			virtual void initialise()
			{

			}

			// Render the window and UI components
			virtual void const onFrame(NVGcontext* context)
			{
				glfwPollEvents();
			}

			// Process events for UI components
			void triggerEventsChain()
			{
				for (OdComponent* control : childComponents)
				{
					control->processEvents(&input);
				}

				// Once the current input states are filtered through all the components
				// reset the 'pressed'/'released' states to prevent double trigger
				if (input.mouse.leftButton.isDown())
					input.mouse.leftButton.changeState(true);

				if (input.mouse.middleButton.isDown())
					input.mouse.middleButton.changeState(true);

				if (input.mouse.rightButton.isDown())
					input.mouse.rightButton.changeState(true);
			}

		};

	} // namespace Graphics
}// namespace OD

#endif // OD_GR_UI_WINDOW_H