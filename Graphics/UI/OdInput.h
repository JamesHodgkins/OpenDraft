#ifndef OD_GR_UI_INPUT_H
#define OD_GR_UI_INPUT_H

/**************************************************************************************
* OpenDraft:    UI Input Class                                                        *
*-------------------------------------------------------------------------------------*
* Filename:     OdInput.h                                                         *
* Contributors: James Hodgkins                                                        *
* Date:         June 9, 2023                                                          *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Inptut map to store the state of inputs against a glfw window instance            *
***************************************************************************************/



#include <unordered_map>
#include "glfw/glfw3.h"
#include <iostream>


namespace OD
{
	namespace Graphics
	{

		class OdKeyInput
		{
		private:
			bool down = false;
			bool pressed = false;
			bool released = false;

		public:

			void changeState(bool aState)
			{
				if (!aState && !down)
					released = false;

				pressed = (!down && aState);
				released = (down && !aState);
				down = aState;
			}

			bool isDown() const { return down; }
			bool isPressDown() const { return pressed; }
			bool isPressUp() const { return released; }
		};


		class GrInputMap
		{
		private:
			struct PositionStruct
			{
				int y;
				int x;
			} position;


		public:

			std::unordered_map<int, OdKeyInput>keys;
			struct MouseStruct
			{
				OdKeyInput leftButton;
				OdKeyInput middleButton;
				OdKeyInput rightButton;
				PositionStruct position;
				int scroll;
			} mouse;

			bool capsLock = false;


			GrInputMap()
			{
				mouse.scroll = 0;
				mouse.position.x = 0;
				mouse.position.y = 0;


				keys = {
					{GLFW_KEY_A, OdKeyInput()},
					{GLFW_KEY_B, OdKeyInput()},
					{GLFW_KEY_C, OdKeyInput()},
					{GLFW_KEY_D, OdKeyInput()},
					{GLFW_KEY_E, OdKeyInput()},
					{GLFW_KEY_F, OdKeyInput()},
					{GLFW_KEY_G, OdKeyInput()},
					{GLFW_KEY_H, OdKeyInput()},
					{GLFW_KEY_I, OdKeyInput()},
					{GLFW_KEY_J, OdKeyInput()},
					{GLFW_KEY_K, OdKeyInput()},
					{GLFW_KEY_L, OdKeyInput()},
					{GLFW_KEY_M, OdKeyInput()},
					{GLFW_KEY_N, OdKeyInput()},
					{GLFW_KEY_O, OdKeyInput()},
					{GLFW_KEY_P, OdKeyInput()},
					{GLFW_KEY_Q, OdKeyInput()},
					{GLFW_KEY_R, OdKeyInput()},
					{GLFW_KEY_S, OdKeyInput()},
					{GLFW_KEY_T, OdKeyInput()},
					{GLFW_KEY_U, OdKeyInput()},
					{GLFW_KEY_V, OdKeyInput()},
					{GLFW_KEY_W, OdKeyInput()},
					{GLFW_KEY_X, OdKeyInput()},
					{GLFW_KEY_Y, OdKeyInput()},
					{GLFW_KEY_Z, OdKeyInput()},
					{GLFW_KEY_0, OdKeyInput()},
					{GLFW_KEY_1, OdKeyInput()},
					{GLFW_KEY_2, OdKeyInput()},
					{GLFW_KEY_3, OdKeyInput()},
					{GLFW_KEY_4, OdKeyInput()},
					{GLFW_KEY_5, OdKeyInput()},
					{GLFW_KEY_6, OdKeyInput()},
					{GLFW_KEY_7, OdKeyInput()},
					{GLFW_KEY_8, OdKeyInput()},
					{GLFW_KEY_9, OdKeyInput()}
				};
			}

		};

	} // namespace Graphics
} // namespace OpenDraft

#endif // !OD_GR_UI_INPUT_H