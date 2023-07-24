#ifndef OD_DAT_DOCUMENTMANAGER_H
#define OD_DAT_DOCUMENTMANAGER_H

/**************************************************************************************
* OpenDraft:    Document Manager Class                                                *
*-------------------------------------------------------------------------------------*
* Filename:     OdDocumentManager.h                                                   *
* Contributors: James Hodgkins                                                        *
* Date:         June 28, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Document Manager class for OpenDraft framework.                                   *
***************************************************************************************/


#include <vector>
#include "System/OdCore.h"
#include "DataManager/OdDocument.h"

using namespace OD::System;

namespace OD
{
	namespace Data
	{
		class OdDocumentManager
		{
		private:
			// Private Properties
			std::vector<OdDocument*> documents;
			int activeDocumentIndex;

		public:
			OdDocumentManager();
			~OdDocumentManager();


			// Methods
			bool openDocument(const char* aFilepath);
			void closeDocument(OdDocument* aDocument);
			void addDocument(OdDocument* aDocument);
			void removeDocument(OdDocument* aDocument);
			void removeDocument(int aIndex);
			void setActiveDocument(OdDocument* aDocument);
			void setActiveDocument(int aIndex);
			OdDocument* getActiveDocument();
			OdDocument* getDocument(int aIndex);
			int getDocumentCount();

		};

	} // namespace OD::Data
} // namespace OD

#endif // !OD_DAT_DOCUMENTMANAGER_H
