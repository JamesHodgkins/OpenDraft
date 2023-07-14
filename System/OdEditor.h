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


#include "System/OdApplication.h"
#include "System/Entities/OdEntity.h"

namespace OD
{
	namespace System
	{
		class OdEditor
		{
		private:
			OdApplication* application;

		public:
			OdEditor(OdApplication* aApplication);

		};
	}
}

#endif // OD_SY_EDITOR_H