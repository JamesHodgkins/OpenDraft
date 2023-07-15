#ifndef OD_SY_EDITOR_H
#define OD_SY_EDITOR_H

/**************************************************************************************
* OpenDraft:    System editor class                                                   *
*-------------------------------------------------------------------------------------*
* Filename:     OdEditor.h                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         14 July, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   An editor class to handle user input/output.                                      *
***************************************************************************************/


#include "System/Entities/OdEntity.h"

namespace OD
{
	// Forward declarations
	namespace Graphics
	{
		class GrInputMap;
	}

	namespace System
	{
		class OdEditor
		{
		private:

		public:
			void processEvents(Graphics::GrInputMap* input);

			OdEditor();

		};
	}
}

#endif // OD_SY_EDITOR_H