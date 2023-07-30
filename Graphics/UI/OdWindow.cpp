#ifndef OD_WINDOW_CPP
#define OD_WINDOW_CPP

/**************************************************************************************
* OpenDraft:    GUI Window Class                                                      *
*-------------------------------------------------------------------------------------*
* Filename:     OdWindow.cpp                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         June 26, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A GUI base window class                                                           *
***************************************************************************************/



#define GLEW_STATIC						// Use static glew library (glew32s.lib)
#include "GL/glew.h"					// Include glew

#include "nanovg.h"						// Include core nanovg library

#ifndef NANOVG_GL3_IMPLEMENTATION
	#define NANOVG_GL3_IMPLEMENTATION	// Use GL3.
	#include "nanovg_gl.h"				// Include nanovg opengl3 implementation
#endif


#include "Graphics/UI/OdWindow.h"
#include "System/OdApplication.h"
#include "System/OdEditor.h"


namespace OD
{
	namespace Graphics
	{
		void OdWindow::updateProperties()
		{
			int width = 0, height = 0;
			glfwGetWindowSize(glfwHandle, &width, &height);
			setSize(width, height);
		}

		void OdWindow::loadResources()
		{
			// To be overridden by derived classes
		}

		OdWindow::OdWindow(int aWidth, int aHeight, const char* aTitle)
		{
			setSize(aWidth, aHeight);
			input = GrInputMap();
			text = aTitle;

			// Initialize GLFW
			if (!glfwInit())
				return;

			// Static cast parameters to float
			int w = static_cast<int>(aWidth);
			int h = static_cast<int>(aHeight);

			// Create window with graphics context
			glfwHandle = glfwCreateWindow(w, h, aTitle, nullptr, nullptr);
			if (glfwHandle == nullptr)
				return;

			glfwMakeContextCurrent(glfwHandle);       // Set context as current
			glfwSwapInterval(1);                      // Enable vsync
			glewInit();                               // Initialize glew

			// Initialize NanoVG context (OpenGL backend)
			//context = nvgCreateGL3(NVG_ANTIALIAS | NVG_STENCIL_STROKES | NVG_DEBUG);
			context = nvgCreateGL3(0);
			if (context == nullptr) {
				return;
			}

			// Bind this instance of GrWindow to the glfw window instance for static callbacks
			glfwSetWindowUserPointer(glfwHandle, this);

			// Set the callback function for mouse movement
			glfwSetCursorPosCallback(glfwHandle, mousePositionEventCallback);
			glfwSetMouseButtonCallback(glfwHandle, mouseClickEventCallback);
			glfwSetScrollCallback(glfwHandle, mouseScrollEventCallback);
			glfwSetKeyCallback(glfwHandle, keyEventCallback);
			glfwSetWindowSizeCallback(glfwHandle, windowResizeCallback);

			// Capture caps lock state
			glfwSetInputMode(glfwHandle, GLFW_LOCK_KEY_MODS, GLFW_TRUE);

			// Update some properties following intitialisation
			updateProperties();
		}

		void OdWindow::mousePositionEventCallback(GLFWwindow* aWindow, double aPositionX, double aPositionY)
		{
			OdWindow* windowInstance = static_cast<OdWindow*>(glfwGetWindowUserPointer(aWindow));
			if (windowInstance) {
				// Access the instance and store the mouse position
				windowInstance->input.mouse.position.x = static_cast<int>(aPositionX);
				windowInstance->input.mouse.position.y = static_cast<int>(aPositionY);
			}
		}

		void OdWindow::windowResizeCallback(GLFWwindow* aWindow, int aWidth, int aHeight)
		{
			OdWindow* windowInstance = static_cast<OdWindow*>(glfwGetWindowUserPointer(aWindow));
			if (windowInstance) {
				// Access the instance and update size
				windowInstance->updateProperties();
				windowInstance->onFrame(windowInstance->context);
			}
		}

		void OdWindow::mouseClickEventCallback(GLFWwindow* aWindow, int aButton, int aAction, int aMods)
		{
			OdWindow* windowInstance = static_cast<OdWindow*>(glfwGetWindowUserPointer(aWindow));
			if (windowInstance) {
				switch (aButton) {
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

		// Callback function for mouse scroll
		void OdWindow::mouseScrollEventCallback(GLFWwindow* aWindow, double aOffsetX, double aOffsetY)
		{
			OdWindow* windowInstance = static_cast<OdWindow*>(glfwGetWindowUserPointer(aWindow));
			if (windowInstance) {
				// Access the instance and store the mouse scroll offset
				windowInstance->input.mouse.scroll = static_cast<int>(aOffsetY);
			}
		}

		void OdWindow::keyEventCallback(GLFWwindow* aWindow, int aKey, int aScancode, int aAction, int aMods)
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


				if (aMods & GLFW_MOD_CAPS_LOCK)
					map->capsLock = true;
				else
					map->capsLock = false;
				

			}
		}

		void OdWindow::setSize(int aWidth, int aHeight)
		{
			size.x = aWidth;
			size.y = aHeight;

			if (glfwHandle)
				glfwSetWindowSize(glfwHandle, aWidth, aHeight);
		}

		bool OdWindow::isRunning()
		{
			return !glfwWindowShouldClose(glfwHandle);
		}

		void OdWindow::makeCurrentContext()
		{
			glfwMakeContextCurrent(glfwHandle);
		}

		NVGcontext* OdWindow::getContext()
		{
			return context;
		}

		OdVector2 OdWindow::getRelativeLocation()
		{
			return OdVector2(0, 0);
		}

		void OdWindow::close()
		{
			glfwDestroyWindow(glfwHandle);
			glfwTerminate();

			childComponents.clear();

			nvgDeleteGL3(context);
		}

		void OdWindow::initialise()
		{

		}

		void OdWindow::onFrame(NVGcontext* context)
		{
			glfwPollEvents();
		}

		void OdWindow::actionEvents(GrInputMap* aInput)
		{

		}

		void OdWindow::triggerEventsChain()
		{
			for (std::shared_ptr<OdComponent> control : childComponents)
			{
				control->processEvents(&input);
			}

			OdApplication* app = OdApplication::getInstance();
			OdDocument* doc = app->getDocumentManager()->getActiveDocument();
		}

		void OdWindow::closeEvents()
		{
			input.mouse.leftButton.changeState(input.mouse.leftButton.isDown());
			input.mouse.middleButton.changeState(input.mouse.middleButton.isDown());
			input.mouse.rightButton.changeState(input.mouse.rightButton.isDown());
			input.mouse.scroll = 0;

			for (auto& pair : input.keys) {
				int key = pair.first;
				input.keys[key].changeState(input.keys[key].isDown());
			}

		}
	} // namespace Graphics
} // namespace OD

#endif // !OD_GR_UI_WINDOW_H
